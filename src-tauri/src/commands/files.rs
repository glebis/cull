use crate::commands::log_library_event;
use crate::db_core::db::Database;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State, Window};

const CLIPBOARD_PASTE_DATE_FORMAT_SETTING: &str = "clipboard_paste_date_format";
const DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT: &str = "%Y-%m-%d";
const PENDING_FOLDER_RENAME_SETTING: &str = "pending_folder_rename";
static FOLDER_RENAME_LOCK: parking_lot::Mutex<()> = parking_lot::const_mutex(());

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingFolderRename {
    source: String,
    target: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenWithApplication {
    name: String,
    path: String,
    is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PastedImageResult {
    path: String,
    image_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenameFolderResult {
    old_path: String,
    new_path: String,
    image_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageFileBytes {
    bytes: Vec<u8>,
    mime_type: String,
}

#[derive(Debug, Clone)]
struct ClipboardImagePayload {
    bytes: Vec<u8>,
    extension: String,
    original_filename: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum DiskMove {
    Rename,
    CopyRemove,
}

fn move_file_on_disk(old_path: &Path, new_path: &Path) -> Result<DiskMove, String> {
    match std::fs::rename(old_path, new_path) {
        Ok(()) => Ok(DiskMove::Rename),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            if let Err(copy_err) = std::fs::copy(old_path, new_path) {
                let _ = std::fs::remove_file(new_path);
                return Err(format!("Failed to copy file across volumes: {}", copy_err));
            }
            if let Err(remove_err) = std::fs::remove_file(old_path) {
                let _ = std::fs::remove_file(new_path);
                return Err(format!(
                    "Failed to remove original after copy: {}",
                    remove_err
                ));
            }
            Ok(DiskMove::CopyRemove)
        }
        Err(e) => Err(format!("Failed to move file: {}", e)),
    }
}

fn rollback_disk_move(kind: DiskMove, old_path: &Path, new_path: &Path) {
    match kind {
        DiskMove::Rename => {
            let _ = std::fs::rename(new_path, old_path);
        }
        DiskMove::CopyRemove => {
            if !old_path.exists() {
                let _ = std::fs::copy(new_path, old_path);
            }
            let _ = std::fs::remove_file(new_path);
        }
    }
}

fn rewrite_folder_descendant(source: &Path, target: &Path, candidate: &str) -> Option<PathBuf> {
    let candidate = Path::new(candidate);
    if candidate == source {
        return Some(target.to_path_buf());
    }
    let relative = candidate.strip_prefix(source).ok()?;
    Some(target.join(relative))
}

#[cfg(target_os = "macos")]
fn rename_directory_exclusive(source: &Path, target: &Path) -> Result<(), String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| "Source path contains an invalid NUL byte".to_string())?;
    let target = CString::new(target.as_os_str().as_bytes())
        .map_err(|_| "Target path contains an invalid NUL byte".to_string())?;
    // SAFETY: both arguments are valid, NUL-terminated path strings and remain
    // alive for the duration of the call. RENAME_EXCL makes the no-overwrite
    // guarantee atomic with the rename itself.
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn rename_directory_exclusive(source: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err("Target folder already exists".to_string());
    }
    std::fs::rename(source, target).map_err(|error| error.to_string())
}

fn restore_watcher_roots(
    watcher: &mut crate::watcher::FileWatcher,
    roots: &[String],
) -> Vec<String> {
    roots
        .iter()
        .filter_map(|root| watcher.watch_folder(root).err())
        .collect()
}

trait FolderWatchOps {
    fn add(&mut self, root: &str) -> Result<(), String>;
    fn remove(&mut self, root: &str) -> Result<(), String>;
}

impl FolderWatchOps for crate::watcher::FileWatcher {
    fn add(&mut self, root: &str) -> Result<(), String> {
        self.watch_folder(root)
    }

    fn remove(&mut self, root: &str) -> Result<(), String> {
        self.unwatch_folder(root)
    }
}

fn register_new_watcher_roots<W: FolderWatchOps>(
    watcher: &mut W,
    roots: &[String],
) -> Result<(), String> {
    let mut registered: Vec<String> = Vec::new();
    for root in roots {
        if let Err(error) = watcher.add(root) {
            let cleanup_errors = registered
                .iter()
                .filter_map(|added| watcher.remove(added).err())
                .collect::<Vec<_>>();
            return if cleanup_errors.is_empty() {
                Err(error)
            } else {
                Err(format!(
                    "{error}; failed to remove partial watcher registrations: {}",
                    cleanup_errors.join("; ")
                ))
            };
        }
        registered.push(root.clone());
    }
    Ok(())
}

fn rollback_folder_rename(
    db: &Database,
    watcher: &parking_lot::Mutex<crate::watcher::FileWatcher>,
    source: &Path,
    target: &Path,
    old_roots: &[String],
    new_roots: &[String],
    disk_moved: bool,
) -> Result<(), String> {
    let mut failures = Vec::new();
    let disk_restored = !disk_moved
        || match rename_directory_exclusive(target, source) {
            Ok(()) => true,
            Err(error) => {
                failures.push(format!("failed to restore folder on disk: {error}"));
                false
            }
        };
    if disk_restored {
        if let Err(error) =
            db.migrate_folder_paths(&target.to_string_lossy(), &source.to_string_lossy())
        {
            failures.push(format!("failed to restore database paths: {error}"));
        }
    }
    let mut watcher = watcher.lock();
    failures.extend(
        restore_watcher_roots(
            &mut watcher,
            if disk_restored { old_roots } else { new_roots },
        )
        .into_iter()
        .map(|error| format!("failed to restore watcher: {error}")),
    );
    if failures.is_empty() {
        db.delete_setting(PENDING_FOLDER_RENAME_SETTING)
            .map_err(|error| format!("failed to clear recovery journal: {error}"))
    } else {
        Err(failures.join("; "))
    }
}

pub fn recover_pending_folder_rename(db: &Database) -> Result<(), String> {
    let Some(raw) = db
        .get_setting(PENDING_FOLDER_RENAME_SETTING)
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    let pending: PendingFolderRename = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid folder rename journal: {error}"))?;
    let source = Path::new(&pending.source);
    let target = Path::new(&pending.target);
    match (source.exists(), target.exists()) {
        (true, false) => {
            db.migrate_folder_paths(&pending.target, &pending.source)
                .map_err(|error| format!("failed to restore database paths: {error}"))?;
        }
        // Journal reservation and the forward DB migration commit in one SQLite
        // transaction, so a surviving journal proves the DB is already forward.
        (false, true) => {}
        (true, true) => {
            return Err(
                "folder rename recovery found both source and target; refusing to choose"
                    .to_string(),
            )
        }
        (false, false) => {
            return Err("folder rename recovery found neither source nor target".to_string())
        }
    }
    db.delete_setting(PENDING_FOLDER_RENAME_SETTING)
        .map_err(|error| format!("failed to clear folder rename journal: {error}"))
}

fn clear_folder_move_intents(
    watcher: &parking_lot::Mutex<crate::watcher::FileWatcher>,
    source: &Path,
    target: &Path,
    files: &[(String, String)],
) {
    let watcher = watcher.lock();
    for (_, old_path) in files {
        if let Some(new_path) = rewrite_folder_descendant(source, target, old_path) {
            watcher.clear_move_intent(Path::new(old_path), &new_path);
        }
    }
}

fn rename_folder_on_disk_and_db(
    db: &Database,
    watcher: &parking_lot::Mutex<crate::watcher::FileWatcher>,
    source: &Path,
    new_name: &str,
) -> Result<RenameFolderResult, String> {
    let _operation_guard = FOLDER_RENAME_LOCK.lock();
    if new_name.is_empty()
        || new_name == "."
        || new_name == ".."
        || new_name.starts_with('.')
        || new_name.contains('/')
        || new_name.contains('\\')
    {
        return Err("Invalid folder name".to_string());
    }
    if !source.is_absolute() || !source.is_dir() {
        return Err("Source folder does not exist".to_string());
    }
    if source.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err("Source folder path must be normalized".to_string());
    }
    if std::fs::symlink_metadata(source)
        .map_err(|error| format!("Failed to inspect source folder: {error}"))?
        .file_type()
        .is_symlink()
    {
        return Err("Source folder cannot be a symbolic link".to_string());
    }
    let parent = source
        .parent()
        .ok_or_else(|| "Cannot rename a filesystem root".to_string())?;
    let target = parent.join(new_name);
    if target == source {
        return Ok(RenameFolderResult {
            old_path: source.to_string_lossy().to_string(),
            new_path: source.to_string_lossy().to_string(),
            image_count: 0,
        });
    }
    if target.exists() {
        return Err(format!("Folder '{}' already exists", new_name));
    }

    let roots = db.list_library_roots().map_err(|error| error.to_string())?;
    let canonical_source = source
        .canonicalize()
        .map_err(|error| format!("Failed to resolve source folder: {error}"))?;
    let source_is_in_library = roots.iter().any(|root| {
        Path::new(root)
            .canonicalize()
            .map(|canonical_root| canonical_source.starts_with(canonical_root))
            .unwrap_or(false)
    });
    let moving_roots = roots
        .iter()
        .filter(|root| Path::new(root).starts_with(source))
        .cloned()
        .collect::<Vec<_>>();
    let safe_managed_parent = !moving_roots.is_empty()
        && std::fs::read_dir(source)
            .map(|entries| {
                entries.into_iter().all(|entry| {
                    entry
                        .map(|entry| {
                            let path = entry.path();
                            entry
                                .file_type()
                                .map(|kind| kind.is_dir() && !kind.is_symlink())
                                .unwrap_or(false)
                                && moving_roots.iter().any(|root| Path::new(root) == path)
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
    if !source_is_in_library && !safe_managed_parent {
        return Err("Source folder is not safely managed by the library".to_string());
    }
    let next_roots = moving_roots
        .iter()
        .filter_map(|root| rewrite_folder_descendant(source, &target, root))
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let files = db
        .list_image_files_under_path(&source.to_string_lossy())
        .map_err(|error| error.to_string())?;

    {
        let mut watcher = watcher.lock();
        let mut unwatched = Vec::new();
        for root in &moving_roots {
            if let Err(error) = watcher.unwatch_folder(root) {
                let restore_errors = restore_watcher_roots(&mut watcher, &unwatched);
                if restore_errors.is_empty() {
                    return Err(error);
                }
                return Err(format!(
                    "{error}; failed to restore watcher: {}",
                    restore_errors.join("; ")
                ));
            }
            unwatched.push(root.clone());
        }
    }

    let pending = PendingFolderRename {
        source: source.to_string_lossy().to_string(),
        target: target.to_string_lossy().to_string(),
    };
    let journal = serde_json::to_string(&pending).map_err(|error| error.to_string())?;
    let migration = match db.migrate_folder_paths_with_journal(
        &source.to_string_lossy(),
        &target.to_string_lossy(),
        PENDING_FOLDER_RENAME_SETTING,
        &journal,
    ) {
        Ok(migration) => migration,
        Err(error) => {
            let restore_errors = restore_watcher_roots(&mut watcher.lock(), &moving_roots);
            let suffix = if restore_errors.is_empty() {
                String::new()
            } else {
                format!("; failed to restore watcher: {}", restore_errors.join("; "))
            };
            return Err(format!(
                "Database path migration/journal reservation failed: {error}{suffix}"
            ));
        }
    };

    {
        let watcher = watcher.lock();
        for (file_id, old_path) in &files {
            if let Some(new_path) = rewrite_folder_descendant(source, &target, old_path) {
                watcher.register_move_intent(PathBuf::from(old_path), new_path, file_id.clone());
            }
        }
    }

    if let Err(error) = rename_directory_exclusive(source, &target) {
        clear_folder_move_intents(watcher, source, &target, &files);
        let rollback = rollback_folder_rename(
            db,
            watcher,
            source,
            &target,
            &moving_roots,
            &next_roots,
            false,
        );
        return Err(match rollback {
            Ok(()) => format!("Failed to rename folder; database restored: {error}"),
            Err(rollback_error) => {
                format!("Failed to rename folder ({error}); rollback also failed: {rollback_error}")
            }
        });
    }

    {
        let mut watcher_guard = watcher.lock();
        if let Err(error) = register_new_watcher_roots(&mut *watcher_guard, &next_roots) {
            drop(watcher_guard);
            let rollback = rollback_folder_rename(
                db,
                watcher,
                source,
                &target,
                &moving_roots,
                &next_roots,
                true,
            );
            clear_folder_move_intents(watcher, source, &target, &files);
            return Err(match rollback {
                Ok(()) => format!("Failed to watch renamed folder; rename restored: {error}"),
                Err(rollback_error) => format!(
                    "Failed to watch renamed folder ({error}); rollback also failed: {rollback_error}"
                ),
            });
        }
    }

    if let Err(error) = db.delete_setting(PENDING_FOLDER_RENAME_SETTING) {
        crate::safe_eprintln!(
            "[folder-rename] Rename committed; recovery journal cleanup will retry at startup: {}",
            error
        );
    }

    Ok(RenameFolderResult {
        old_path: source.to_string_lossy().to_string(),
        new_path: target.to_string_lossy().to_string(),
        image_count: migration.image_files,
    })
}

fn mime_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "apng" | "png" => "image/png",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn image_file_bytes_for_id(db: &Database, image_id: &str) -> Result<ImageFileBytes, String> {
    let images = db
        .get_images_by_ids(&[image_id])
        .map_err(|e| e.to_string())?;
    let img = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;
    let path = PathBuf::from(&img.path);
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("Failed to read original image '{}': {}", image_id, e))?;
    Ok(ImageFileBytes {
        bytes,
        mime_type: mime_type_for_path(&path).to_string(),
    })
}

pub(crate) fn resolve_image_original_path_for_db(
    db: &Database,
    image_id: &str,
) -> Result<String, String> {
    let candidates = db
        .original_file_candidates(image_id)
        .map_err(|error| error.to_string())?;
    if let Some((path, _is_referenced)) = candidates
        .into_iter()
        .find(|(path, _is_referenced)| Path::new(path).exists())
    {
        return Ok(path);
    }

    if let Some(source) = db
        .referenced_source_for_image(image_id)
        .map_err(|error| error.to_string())?
    {
        if source.offline_at.is_some() {
            return Err(format!(
                "Reconnect {} to open originals",
                source.display_name
            ));
        }
    }

    Err(format!("Image '{image_id}' has no available original"))
}

fn sanitize_extension(extension: &str) -> String {
    let cleaned: String = extension
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if cleaned.is_empty() {
        "png".to_string()
    } else {
        cleaned
    }
}

fn split_numeric_suffix(stem: &str) -> Option<(&str, &str)> {
    let digits_start = stem
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(0);
    if digits_start >= stem.len() {
        return None;
    }
    Some((&stem[..digits_start], &stem[digits_start..]))
}

fn folder_wide_numeric_sequence(file_names: &[String], extension: &str) -> Option<String> {
    let mut prefix: Option<String> = None;
    let mut width: Option<usize> = None;
    let mut max_number = 0u64;
    let mut matched = 0usize;

    for file_name in file_names {
        let path = Path::new(file_name);
        let Some(file_ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !file_ext.eq_ignore_ascii_case(extension) {
            continue;
        }
        let stem = path.file_stem().and_then(|stem| stem.to_str())?;
        let (candidate_prefix, digits) = split_numeric_suffix(stem)?;
        if digits.is_empty() {
            return None;
        }
        let candidate_width = digits.len();
        let number = digits.parse::<u64>().ok()?;

        match (&prefix, width) {
            (Some(existing_prefix), Some(existing_width))
                if existing_prefix == candidate_prefix && existing_width == candidate_width => {}
            (None, None) => {
                prefix = Some(candidate_prefix.to_string());
                width = Some(candidate_width);
            }
            _ => return None,
        }

        matched += 1;
        max_number = max_number.max(number);
    }

    let prefix = prefix?;
    let width = width?;
    if matched == 0 {
        return None;
    }
    Some(format!(
        "{}{:0width$}.{}",
        prefix,
        max_number + 1,
        extension,
        width = width
    ))
}

fn sanitize_filename_part(value: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        let next = if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_whitespace() || ch == '/' || ch == '\\' {
            Some('-')
        } else {
            None
        };

        if let Some(ch) = next {
            if ch == '-' {
                if !last_dash && !out.is_empty() {
                    out.push(ch);
                }
                last_dash = true;
            } else {
                out.push(ch);
                last_dash = false;
            }
        }
    }

    let cleaned = out.trim_matches(['-', '.', '_']).to_string();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

fn unique_filename(directory: &Path, base: &str, extension: &str) -> String {
    let first = format!("{}.{}", base, extension);
    if !directory.join(&first).exists() {
        return first;
    }

    for n in 2.. {
        let candidate = format!("{}-{:02}.{}", base, n, extension);
        if !directory.join(&candidate).exists() {
            return candidate;
        }
    }
    unreachable!("unbounded filename counter should always find a candidate")
}

fn next_paste_filename(
    directory: &Path,
    extension: &str,
    original_filename: Option<&str>,
    date_prefix: &str,
) -> Result<String, String> {
    let extension = sanitize_extension(extension);
    let file_names = std::fs::read_dir(directory)
        .map_err(|e| format!("Failed to read destination folder: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ty| ty.is_file()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().to_str().map(|name| name.to_string()))
        .collect::<Vec<_>>();

    if let Some(candidate) = folder_wide_numeric_sequence(&file_names, &extension) {
        if !directory.join(&candidate).exists() {
            return Ok(candidate);
        }
    }

    let date = sanitize_filename_part(date_prefix, "pasted");
    let source = original_filename
        .and_then(|name| {
            Path::new(name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| sanitize_filename_part(stem, "image"))
        })
        .unwrap_or_else(|| "image".to_string());
    let base = format!("{}-{}", date, source);

    Ok(unique_filename(directory, &base, &extension))
}

fn render_path_as_png_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let image = image::open(path).map_err(|e| format!("Failed to decode image: {}", e))?;
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image for clipboard: {}", e))?;
    Ok(bytes)
}

fn target_is_in_library(destination: &Path, roots: &[String]) -> bool {
    let dest_canonical =
        std::fs::canonicalize(destination).unwrap_or_else(|_| destination.to_path_buf());
    roots.iter().any(|root| {
        let root_path = PathBuf::from(root);
        let root_canonical = std::fs::canonicalize(&root_path).unwrap_or(root_path);
        dest_canonical.starts_with(&root_canonical)
    })
}

fn clipboard_date_prefix(state: &AppState) -> String {
    let format = state
        .db
        .get_setting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT.to_string());
    chrono::Local::now().format(&format).to_string()
}

#[cfg(target_os = "macos")]
fn nsdata_to_vec(data: &objc2_foundation::NSData) -> Vec<u8> {
    let len = data.length();
    let mut bytes = vec![0u8; len];
    if len > 0 {
        let ptr =
            std::ptr::NonNull::new(bytes.as_mut_ptr().cast()).expect("vec pointer is not null");
        unsafe { data.getBytes_length(ptr, len) };
    }
    bytes
}

#[cfg(target_os = "macos")]
fn read_string_for_pasteboard_type(
    pasteboard: &objc2_app_kit::NSPasteboard,
    ty: &objc2_app_kit::NSPasteboardType,
) -> Option<String> {
    pasteboard.stringForType(ty).map(|value| value.to_string())
}

#[cfg(target_os = "macos")]
fn read_file_url_from_pasteboard(pasteboard: &objc2_app_kit::NSPasteboard) -> Option<PathBuf> {
    use objc2_app_kit::NSPasteboardTypeFileURL;
    use objc2_foundation::{NSString, NSURL};

    let file_url = read_string_for_pasteboard_type(pasteboard, unsafe { NSPasteboardTypeFileURL })?;
    let url = NSURL::URLWithString(&NSString::from_str(&file_url))?;
    url.to_file_path()
}

#[cfg(target_os = "macos")]
fn read_image_from_clipboard() -> Result<Option<ClipboardImagePayload>, String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeTIFF};

    let pasteboard = NSPasteboard::generalPasteboard();

    if let Some(path) = read_file_url_from_pasteboard(&pasteboard) {
        // Intentional exception to the module_raw default-on policy (bd
        // imageview-dkz.12): this pasteboard helper has no DB/settings access,
        // and clipboard-pasted RAW file URLs stay excluded regardless of the
        // setting. Folder import and the watcher honor is_module_raw_enabled.
        let module_raw = false;
        if crate::extensions::is_image_path(&path, module_raw) && path.exists() {
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read clipboard file URL: {}", e))?;
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("png")
                .to_string();
            let original_filename = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string());
            return Ok(Some(ClipboardImagePayload {
                bytes,
                extension,
                original_filename,
            }));
        }
    }

    if let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypePNG }) {
        return Ok(Some(ClipboardImagePayload {
            bytes: nsdata_to_vec(&data),
            extension: "png".to_string(),
            original_filename: None,
        }));
    }

    if let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypeTIFF }) {
        return Ok(Some(ClipboardImagePayload {
            bytes: nsdata_to_vec(&data),
            extension: "tiff".to_string(),
            original_filename: None,
        }));
    }

    Ok(None)
}

#[cfg(not(target_os = "macos"))]
fn read_image_from_clipboard() -> Result<Option<ClipboardImagePayload>, String> {
    Err("Image clipboard paste is currently available on macOS only".to_string())
}

#[cfg(target_os = "macos")]
fn copy_path_to_clipboard(path: &Path) -> Result<(), String> {
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
        NSPasteboardTypeURL,
    };
    use objc2_foundation::{NSData, NSString, NSURL};

    let url = NSURL::from_file_path(path)
        .ok_or_else(|| format!("Invalid image path for clipboard: {}", path.display()))?;
    let url_string = url
        .absoluteString()
        .ok_or_else(|| format!("Invalid file URL for clipboard: {}", path.display()))?
        .to_string();
    let path_string = path.to_string_lossy().to_string();

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();

    let url_ns = NSString::from_str(&url_string);
    let mut wrote = pasteboard.setString_forType(&url_ns, unsafe { NSPasteboardTypeFileURL });
    wrote |= pasteboard.setString_forType(&url_ns, unsafe { NSPasteboardTypeURL });
    let path_ns = NSString::from_str(&path_string);
    let _ = pasteboard.setString_forType(&path_ns, unsafe { NSPasteboardTypeString });

    if let Ok(png_bytes) = render_path_as_png_bytes(path) {
        let data = NSData::with_bytes(&png_bytes);
        wrote |= pasteboard.setData_forType(Some(&data), unsafe { NSPasteboardTypePNG });
    }

    if wrote {
        Ok(())
    } else {
        Err("Failed to write image to clipboard".to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn copy_path_to_clipboard(_path: &Path) -> Result<(), String> {
    Err("Image clipboard copy is currently available on macOS only".to_string())
}

#[tauri::command]
pub async fn copy_image_to_clipboard(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<(), String> {
    let images = state
        .db
        .get_images_by_ids(&[&image_id])
        .map_err(|e| e.to_string())?;
    let img = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;
    let path = PathBuf::from(&img.path);
    if !path.exists() {
        return Err(format!("Cannot copy missing file: {}", img.path));
    }

    copy_path_to_clipboard(&path)
}

#[tauri::command]
pub async fn get_image_file_bytes(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<ImageFileBytes, String> {
    image_file_bytes_for_id(&state.db, &image_id)
}

#[tauri::command]
pub async fn paste_image_from_clipboard(
    app: AppHandle,
    state: State<'_, AppState>,
    destination_folder: String,
    session_id: Option<String>,
) -> Result<PastedImageResult, String> {
    let destination = PathBuf::from(&destination_folder);
    if !destination.is_dir() {
        return Err("Destination folder does not exist".to_string());
    }
    // Apply the shared path policy: a paste destination is an explicitly chosen
    // folder, but must still never be a sensitive directory (e.g. ~/.ssh).
    crate::db_core::path_policy::validate_path(
        &destination_folder,
        crate::db_core::path_policy::PathMode::UserPicked,
    )?;

    let payload = read_image_from_clipboard()?
        .ok_or_else(|| "Clipboard does not contain an image".to_string())?;
    let date_prefix = clipboard_date_prefix(&state);
    let filename = next_paste_filename(
        &destination,
        &payload.extension,
        payload.original_filename.as_deref(),
        &date_prefix,
    )?;
    let target = destination.join(filename);
    std::fs::write(&target, &payload.bytes)
        .map_err(|e| format!("Failed to write pasted image: {}", e))?;

    let image_id = crate::db_core::import::import_file(&state.db, &target, &state.app_data_dir)?;

    if let (Some(active_session_id), Some(image_id)) = (session_id.as_deref(), image_id.as_deref())
    {
        let _ = state.db.add_to_collection(active_session_id, &[image_id]);
    }

    let target_str = target.to_string_lossy().to_string();
    // Do NOT widen the asset: protocol scope to the pasted original. The renderer
    // displays pasted images through the app-owned thumbnail generated by
    // import_file above (under $APPDATA/thumbnails, already in the static scope);
    // the frontend reloads the grid after paste. Granting asset: access to the
    // original here would breach the file-access boundary documented in SECURITY.md.

    let roots = state.db.list_library_roots().map_err(|e| e.to_string())?;
    if !target_is_in_library(&destination, &roots) {
        if let Err(e) = state.db.add_library_root(&destination_folder) {
            crate::safe_eprintln!(
                "[files] Failed to add paste destination as library root: {}",
                e
            );
        } else {
            let mut fw = state.file_watcher.lock();
            let _ = fw.watch_folder(&destination_folder);
            let _ = app.emit("folders:changed", ());
        }
    }

    let _ = app.emit("images:changed", ());

    if let Some(image_id) = image_id.as_ref() {
        log_library_event(
            &state,
            "clipboard_image_pasted",
            Some("image"),
            Some(image_id.clone()),
            serde_json::json!({
                "image_id": image_id,
                "path": target_str.clone(),
                "destination_folder": destination_folder,
                "session_id": session_id,
            }),
        );
    }

    Ok(PastedImageResult {
        path: target_str,
        image_id,
    })
}

#[tauri::command]
pub async fn move_image(
    app: AppHandle,
    state: State<'_, AppState>,
    image_id: String,
    destination_folder: String,
) -> Result<String, String> {
    state
        .db
        .ensure_original_mutation_allowed(&image_id)
        .map_err(|error| error.to_string())?;
    let images = state
        .db
        .get_images_by_ids(&[&image_id])
        .map_err(|e| e.to_string())?;
    let img = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;

    let old_path = PathBuf::from(&img.path);
    let filename = old_path.file_name().ok_or("Invalid source path")?;
    let destination = PathBuf::from(&destination_folder);

    if !destination.is_dir() {
        return Err("Destination folder does not exist".to_string());
    }

    let roots = state.db.list_library_roots().map_err(|e| e.to_string())?;
    let dest_canonical =
        std::fs::canonicalize(&destination).unwrap_or_else(|_| destination.clone());
    let in_library = roots.iter().any(|root| {
        let root_canonical = std::fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));
        dest_canonical.starts_with(&root_canonical)
    });
    let new_path = destination.join(filename);

    if new_path.exists() {
        if new_path == old_path {
            return Ok(img.path.clone());
        }
        return Err(format!("File already exists at {}", new_path.display()));
    }

    let file_record = state
        .db
        .get_image_file_by_path(&img.path)
        .map_err(|e| e.to_string())?
        .ok_or("Image file record not found")?;

    {
        let fw = state.file_watcher.lock();
        fw.register_move_intent(old_path.clone(), new_path.clone(), file_record.id.clone());
    }

    let disk_move = move_file_on_disk(&old_path, &new_path)?;

    let new_path_str = new_path.to_string_lossy().to_string();
    if let Err(e) = state
        .db
        .update_image_file_path(&file_record.id, &new_path_str)
    {
        rollback_disk_move(disk_move, &old_path, &new_path);
        return Err(format!("DB update failed, file moved back: {}", e));
    }

    if !in_library {
        if let Err(e) = state.db.add_library_root(&destination_folder) {
            crate::safe_eprintln!(
                "[files] Failed to add move destination as library root: {}",
                e
            );
        } else {
            let mut fw = state.file_watcher.lock();
            let _ = fw.watch_folder(&destination_folder);
            let _ = app.emit("folders:changed", ());
        }
    }

    let _ = app.emit("images:changed", ());

    log_library_event(
        &state,
        "image_moved",
        Some("image"),
        Some(image_id.clone()),
        serde_json::json!({
            "image_id": image_id,
            "old_path": old_path.to_string_lossy(),
            "new_path": new_path_str.clone(),
            "destination_folder": destination_folder,
        }),
    );

    Ok(new_path_str)
}

#[tauri::command]
pub async fn rename_image(
    app: AppHandle,
    state: State<'_, AppState>,
    image_id: String,
    new_name: String,
) -> Result<String, String> {
    state
        .db
        .ensure_original_mutation_allowed(&image_id)
        .map_err(|error| error.to_string())?;
    if new_name.is_empty() || new_name.contains('/') || new_name.contains('\\') {
        return Err("Invalid filename".to_string());
    }

    let images = state
        .db
        .get_images_by_ids(&[&image_id])
        .map_err(|e| e.to_string())?;
    let img = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;

    let old_path = PathBuf::from(&img.path);
    let parent = old_path.parent().ok_or("Invalid source path")?;
    let new_path = parent.join(&new_name);

    if new_path == old_path {
        return Ok(img.path.clone());
    }

    if new_path.exists() {
        return Err(format!("File '{}' already exists", new_name));
    }

    let file_record = state
        .db
        .get_image_file_by_path(&img.path)
        .map_err(|e| e.to_string())?
        .ok_or("Image file record not found")?;

    {
        let fw = state.file_watcher.lock();
        fw.register_move_intent(old_path.clone(), new_path.clone(), file_record.id.clone());
    }

    std::fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename file: {}", e))?;

    let new_path_str = new_path.to_string_lossy().to_string();
    if let Err(e) = state
        .db
        .update_image_file_path(&file_record.id, &new_path_str)
    {
        let _ = std::fs::rename(&new_path, &old_path);
        return Err(format!("DB update failed, file renamed back: {}", e));
    }

    let _ = app.emit("images:changed", ());

    log_library_event(
        &state,
        "image_renamed",
        Some("image"),
        Some(image_id.clone()),
        serde_json::json!({
            "image_id": image_id,
            "old_path": old_path.to_string_lossy(),
            "new_path": new_path_str.clone(),
            "new_name": new_name,
        }),
    );

    Ok(new_path_str)
}

#[tauri::command]
pub async fn create_subfolder(
    app: AppHandle,
    state: State<'_, AppState>,
    parent_path: String,
    name: String,
) -> Result<String, String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.') {
        return Err("Invalid folder name".to_string());
    }

    let roots = state.db.list_library_roots().map_err(|e| e.to_string())?;
    let parent_canonical =
        std::fs::canonicalize(&parent_path).unwrap_or_else(|_| PathBuf::from(&parent_path));
    let in_library = roots.iter().any(|root| {
        let root_canonical = std::fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));
        parent_canonical.starts_with(&root_canonical)
    });
    if !in_library {
        return Err("Parent folder is not within a library root".to_string());
    }

    let new_folder = PathBuf::from(&parent_path).join(&name);
    if new_folder.exists() {
        return Err(format!("Folder '{}' already exists", name));
    }

    std::fs::create_dir(&new_folder).map_err(|e| format!("Failed to create folder: {}", e))?;

    {
        let mut fw = state.file_watcher.lock();
        let _ = fw.watch_folder(&new_folder.to_string_lossy());
    }

    let _ = app.emit("folders:changed", ());

    let new_folder_str = new_folder.to_string_lossy().to_string();
    log_library_event(
        &state,
        "folder_created",
        Some("folder"),
        Some(new_folder_str.clone()),
        serde_json::json!({
            "parent_path": parent_path,
            "name": name,
            "path": new_folder_str.clone(),
        }),
    );

    Ok(new_folder_str)
}

#[tauri::command]
pub async fn rename_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: String,
    new_name: String,
) -> Result<RenameFolderResult, String> {
    let result = rename_folder_on_disk_and_db(
        &state.db,
        &state.file_watcher,
        Path::new(&folder),
        &new_name,
    )?;
    let _ = app.emit("folders:changed", ());
    let _ = app.emit("images:changed", ());
    log_library_event(
        &state,
        "folder_renamed",
        Some("folder"),
        Some(result.new_path.clone()),
        serde_json::json!({
            "old_path": result.old_path,
            "new_path": result.new_path,
            "image_count": result.image_count,
        }),
    );
    Ok(result)
}

#[tauri::command]
pub async fn share_images(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<(), String> {
    if image_ids.is_empty() {
        return Err("No images selected to share".to_string());
    }

    let id_refs: Vec<&str> = image_ids.iter().map(|id| id.as_str()).collect();
    let found = state
        .db
        .get_images_by_ids(&id_refs)
        .map_err(|e| e.to_string())?;
    if found.is_empty() {
        return Err("No matching images found to share".to_string());
    }

    let mut paths = Vec::with_capacity(found.len());
    for img in found {
        let path = PathBuf::from(&img.path);
        if !path.exists() {
            return Err(format!("Cannot share missing file: {}", img.path));
        }
        paths.push(path);
    }

    share_paths(app, window.label().to_string(), paths)
}

#[tauri::command]
pub async fn open_images_with_application(
    state: State<'_, AppState>,
    app_path: String,
    image_ids: Vec<String>,
) -> Result<(), String> {
    if image_ids.is_empty() {
        return Err("No image selected to open".to_string());
    }
    if image_ids.len() > 1 {
        return Err("Open With currently supports one image at a time".to_string());
    }

    let app_bundle = PathBuf::from(&app_path);
    validate_app_bundle(&app_bundle)?;

    let path = PathBuf::from(resolve_image_original_path_for_db(
        &state.db,
        &image_ids[0],
    )?);

    open_paths_with_application(&app_bundle, vec![path])
}

#[tauri::command]
pub async fn resolve_image_original_path(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<String, String> {
    resolve_image_original_path_for_db(&state.db, &image_id)
}

#[tauri::command]
pub async fn list_open_with_applications(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Vec<OpenWithApplication>, String> {
    let path = PathBuf::from(resolve_image_original_path_for_db(&state.db, &image_id)?);

    list_applications_for_path(&path)
}

fn validate_app_bundle(app_path: &Path) -> Result<(), String> {
    if !app_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("app"))
        .unwrap_or(false)
    {
        return Err("Choose a macOS .app bundle".to_string());
    }
    if !app_path.exists() {
        return Err(format!("Application not found: {}", app_path.display()));
    }
    if !app_path.is_dir() {
        return Err("Choose a macOS .app bundle".to_string());
    }

    // Canonicalize to resolve symlinks and ../ traversal before checking the allowlist
    let canonical = app_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve application path: {}", e))?;

    let home_apps = dirs::home_dir()
        .map(|h| h.join("Applications"))
        .unwrap_or_else(|| PathBuf::from("/Users/Shared/Applications"));

    let allowed_prefixes: Vec<PathBuf> = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Library"),
        home_apps,
    ];

    let in_allowed_dir = allowed_prefixes
        .iter()
        .any(|prefix| canonical.starts_with(prefix));

    if !in_allowed_dir {
        return Err(format!(
            "Application '{}' is outside allowed directories. \
             Only apps in /Applications, /System/Applications, \
             ~/Applications, or /System/Library are permitted.",
            app_path.display()
        ));
    }

    Ok(())
}

fn app_display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Application")
        .to_string()
}

#[cfg(target_os = "macos")]
fn open_paths_with_application(app_path: &Path, paths: Vec<PathBuf>) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .arg("-a")
        .arg(app_path)
        .arg("--")
        .args(paths.iter())
        .status()
        .map_err(|e| format!("Failed to launch application: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Open With failed for {} with status {}",
            app_path.display(),
            status
        ))
    }
}

#[cfg(target_os = "macos")]
fn list_applications_for_path(path: &Path) -> Result<Vec<OpenWithApplication>, String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSURL;

    let file_url = NSURL::from_file_path(path)
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;
    let workspace = NSWorkspace::sharedWorkspace();
    let default_path = workspace
        .URLForApplicationToOpenURL(&file_url)
        .and_then(|url| url.to_file_path());
    let app_urls = workspace.URLsForApplicationsToOpenURL(&file_url);

    let mut seen = HashSet::new();
    let mut default_apps = Vec::new();
    let mut other_apps = Vec::new();
    for app_url in app_urls.to_vec() {
        let Some(path) = app_url.to_file_path() else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let path_str = path.to_string_lossy().to_string();
        if !seen.insert(path_str.clone()) {
            continue;
        }
        let app = OpenWithApplication {
            name: app_display_name(&path),
            is_default: default_path
                .as_ref()
                .is_some_and(|default| default == &path),
            path: path_str,
        };
        if app.is_default {
            default_apps.push(app);
        } else {
            other_apps.push(app);
        }
    }

    default_apps.extend(other_apps);
    Ok(default_apps)
}

#[cfg(not(target_os = "macos"))]
fn open_paths_with_application(_app_path: &Path, _paths: Vec<PathBuf>) -> Result<(), String> {
    Err("Open With is currently available on macOS only".to_string())
}

#[cfg(not(target_os = "macos"))]
fn list_applications_for_path(_path: &Path) -> Result<Vec<OpenWithApplication>, String> {
    Err("Open With application discovery is currently available on macOS only".to_string())
}

#[cfg(target_os = "macos")]
fn share_paths(app: AppHandle, window_label: String, paths: Vec<PathBuf>) -> Result<(), String> {
    if objc2::MainThreadMarker::new().is_some() {
        return show_share_picker_on_main(&app, &window_label, &paths);
    }

    let handle = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let result = show_share_picker_on_main(&handle, &window_label, &paths);
        let _ = tx.send(result);
    })
    .map_err(|e| format!("Failed to schedule share sheet: {}", e))?;

    rx.recv()
        .map_err(|_| "Failed to receive share sheet result".to_string())?
}

#[cfg(target_os = "macos")]
fn show_share_picker_on_main(
    app: &AppHandle,
    window_label: &str,
    paths: &[PathBuf],
) -> Result<(), String> {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSSharingServicePicker, NSView};
    use objc2_foundation::{NSArray, NSRectEdge, NSURL};
    use tauri::Manager;

    let _mtm = objc2::MainThreadMarker::new()
        .ok_or_else(|| "macOS share sheet must run on the main thread".to_string())?;
    let window = app
        .get_webview_window(window_label)
        .ok_or_else(|| format!("Window '{}' not found", window_label))?;
    let ns_view = window
        .ns_view()
        .map_err(|e| format!("Failed to access native view: {}", e))?;
    let view = unsafe { (ns_view as *mut NSView).as_ref() }
        .ok_or_else(|| "Native view is unavailable".to_string())?;

    let urls = paths
        .iter()
        .map(|path| {
            NSURL::from_file_path(path)
                .ok_or_else(|| format!("Could not create file URL for {}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let url_refs = urls.iter().map(|url| url.as_ref()).collect::<Vec<&NSURL>>();
    let items = NSArray::from_slice(&url_refs);
    let picker = unsafe {
        NSSharingServicePicker::initWithItems(
            NSSharingServicePicker::alloc(),
            items.cast_unchecked(),
        )
    };

    picker.showRelativeToRect_ofView_preferredEdge(view.bounds(), view, NSRectEdge::NSMinYEdge);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn share_paths(_app: AppHandle, _window_label: String, _paths: Vec<PathBuf>) -> Result<(), String> {
    Err("System sharing is currently available on macOS only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::{Image, ImageFile, ReferencedSource, ReferencedSourceKind};
    use image::{ImageBuffer, Rgba};
    use std::io::Write;

    fn insert_original_candidate(
        db: &Database,
        image_id: &str,
        file_id: &str,
        path: &std::path::Path,
    ) {
        db.insert_image(&Image {
            id: image_id.to_string(),
            sha256_hash: format!("hash-{image_id}"),
            width: 1,
            height: 1,
            format: "jpg".to_string(),
            file_size: 1,
            created_at: "2026-09-01T10:00:00Z".to_string(),
            imported_at: "2026-09-01T10:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: file_id.to_string(),
            image_id: image_id.to_string(),
            path: path.to_string_lossy().to_string(),
            last_seen_at: "2026-09-01T10:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
    }

    fn offline_referenced_source() -> ReferencedSource {
        ReferencedSource {
            id: "source-untitled".to_string(),
            platform_volume_id: Some("volume-untitled".to_string()),
            display_name: "UNTITLED".to_string(),
            last_mount_path: Some("/Volumes/UNTITLED".to_string()),
            source_kind: ReferencedSourceKind::SdCard,
            capacity_bytes: None,
            recursive_default: false,
            settings_json: "{}".to_string(),
            last_seen_at: "2026-09-01T10:00:00Z".to_string(),
            offline_at: Some("2026-09-01T11:00:00Z".to_string()),
        }
    }

    #[derive(Default)]
    struct FailingWatchOps {
        added: Vec<String>,
        removed: Vec<String>,
        fail_on: String,
    }

    impl FolderWatchOps for FailingWatchOps {
        fn add(&mut self, root: &str) -> Result<(), String> {
            if root == self.fail_on {
                return Err(format!("cannot watch {root}"));
            }
            self.added.push(root.to_string());
            Ok(())
        }

        fn remove(&mut self, root: &str) -> Result<(), String> {
            self.removed.push(root.to_string());
            Ok(())
        }
    }

    /// Security regression guard for SECURITY.md's asset-protocol boundary:
    /// the `asset:` scope is configured statically in tauri.conf.json
    /// (thumbnails / generated only). No code may widen it at runtime, which
    /// would silently expose user originals to the renderer. This scans the
    /// entire Rust source tree so the boundary cannot be reopened in any file.
    #[test]
    fn no_runtime_asset_protocol_scope_expansion_in_source() {
        fn collect_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect_rs(&path, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(path);
                }
            }
        }

        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs(&src_dir, &mut files);
        assert!(!files.is_empty(), "found no .rs files to scan");

        // Patterns that grant the renderer filesystem access at runtime.
        let forbidden = ["asset_protocol_scope", "allow_file", "allow_directory"];
        let mut offenders = Vec::new();

        for file in &files {
            let contents = std::fs::read_to_string(file).unwrap();
            for (lineno, line) in contents.lines().enumerate() {
                let trimmed = line.trim_start();
                // Ignore comments and this guard's own pattern strings.
                if trimmed.starts_with("//") || trimmed.starts_with("* ") {
                    continue;
                }
                if line.contains("ALLOWLIST-asset-scope") {
                    continue; // explicit, audited opt-out marker
                }
                for pat in &forbidden {
                    // Match a call site `.<pat>(`, not the literal string in this test.
                    if line.contains(&format!(".{}(", pat)) {
                        offenders.push(format!(
                            "{}:{}: {}",
                            file.strip_prefix(&src_dir).unwrap().display(),
                            lineno + 1,
                            trimmed
                        ));
                    }
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "runtime asset-protocol scope expansion found (widens the renderer's \
             file access beyond tauri.conf.json). Render via app-owned thumbnails \
             instead, or add an audited `ALLOWLIST-asset-scope` marker:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn move_file_on_disk_renames_within_same_volume() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.png");
        let dest = dir.path().join("dest.png");
        {
            let mut file = std::fs::File::create(&source).unwrap();
            file.write_all(b"image").unwrap();
        }

        let kind = move_file_on_disk(&source, &dest).unwrap();

        assert_eq!(kind, DiskMove::Rename);
        assert!(!source.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"image");
    }

    #[test]
    fn image_file_bytes_for_id_reads_original_file() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data_dir = tmp.path().join("app-data");
        std::fs::create_dir(&app_data_dir).unwrap();
        let image_path = tmp.path().join("full.png");
        let image = ImageBuffer::from_fn(4, 4, |x, y| {
            Rgba([(x * 32) as u8, (y * 32) as u8, 128, 255])
        });
        image.save(&image_path).unwrap();
        let original_bytes = std::fs::read(&image_path).unwrap();

        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image_id = crate::db_core::import::import_file(&db, &image_path, &app_data_dir)
            .unwrap()
            .unwrap();

        let payload = image_file_bytes_for_id(&db, &image_id).unwrap();

        assert_eq!(payload.mime_type, "image/png");
        assert_eq!(payload.bytes, original_bytes);
    }

    #[test]
    fn image_file_bytes_for_id_rejects_unknown_image() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let err = image_file_bytes_for_id(&db, "missing").unwrap_err();
        assert!(err.contains("Image 'missing' not found"));
    }

    #[test]
    fn original_resolution_prefers_an_available_normal_file() {
        let tmp = tempfile::tempdir().unwrap();
        let local_path = tmp.path().join("local.jpg");
        std::fs::write(&local_path, b"local original").unwrap();
        let referenced_path = tmp.path().join("referenced.jpg");
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();

        insert_original_candidate(&db, "mixed", "mixed-local", &local_path);
        db.insert_image_file(&ImageFile {
            id: "mixed-referenced".to_string(),
            image_id: "mixed".to_string(),
            path: referenced_path.to_string_lossy().to_string(),
            last_seen_at: "2026-09-01T10:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
        db.upsert_referenced_source(&offline_referenced_source())
            .unwrap();
        db.attach_referenced_file("source-untitled", "mixed-referenced", "referenced.jpg")
            .unwrap();

        assert_eq!(
            resolve_image_original_path_for_db(&db, "mixed").unwrap(),
            local_path.to_string_lossy()
        );
    }

    #[test]
    fn offline_referenced_original_names_the_source_to_reconnect() {
        let tmp = tempfile::tempdir().unwrap();
        let unavailable_path = tmp.path().join("offline.jpg");
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();

        insert_original_candidate(&db, "offline-only", "offline-file", &unavailable_path);
        db.upsert_referenced_source(&offline_referenced_source())
            .unwrap();
        db.attach_referenced_file("source-untitled", "offline-file", "offline.jpg")
            .unwrap();

        assert_eq!(
            resolve_image_original_path_for_db(&db, "offline-only").unwrap_err(),
            "Reconnect UNTITLED to open originals"
        );
    }

    #[test]
    fn rollback_disk_move_restores_rename() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.png");
        let dest = dir.path().join("dest.png");
        std::fs::write(&dest, b"image").unwrap();

        rollback_disk_move(DiskMove::Rename, &source, &dest);

        assert_eq!(std::fs::read(&source).unwrap(), b"image");
        assert!(!dest.exists());
    }

    #[test]
    fn rename_folder_moves_disk_and_database_paths_together() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let nested = source.join("nested");
        let app_data = tmp.path().join("app-data");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(&app_data).unwrap();
        let image_path = nested.join("image.png");
        ImageBuffer::from_pixel(2, 2, Rgba([10u8, 20, 30, 255]))
            .save(&image_path)
            .unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image_id = crate::db_core::import::import_file(&db, &image_path, &app_data)
            .unwrap()
            .unwrap();
        db.set_decision(&image_id, "accept").unwrap();
        db.add_library_root(&source.to_string_lossy()).unwrap();
        let watcher = parking_lot::Mutex::new(crate::watcher::FileWatcher::new());

        let result = rename_folder_on_disk_and_db(&db, &watcher, &source, "renamed").unwrap();

        let renamed_image = tmp.path().join("renamed/nested/image.png");
        assert!(!source.exists());
        assert!(renamed_image.exists());
        assert_eq!(
            result.new_path,
            tmp.path().join("renamed").to_string_lossy()
        );
        assert!(db
            .get_image_file_by_path(&renamed_image.to_string_lossy())
            .unwrap()
            .is_some());
        assert_eq!(
            db.get_selection_for_image(&image_id)
                .unwrap()
                .unwrap()
                .decision,
            "accept"
        );
        assert_eq!(
            db.list_library_roots().unwrap(),
            vec![tmp.path().join("renamed").to_string_lossy().to_string()]
        );
    }

    #[test]
    fn rename_folder_leaves_disk_untouched_when_database_paths_collide() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let source_file = source.join("image.png");
        std::fs::write(&source_file, b"source").unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let inside = crate::db_core::models::Image {
            id: "inside".to_string(),
            sha256_hash: "inside-hash".to_string(),
            width: 1,
            height: 1,
            format: "png".to_string(),
            file_size: 6,
            created_at: "2026-01-01".to_string(),
            imported_at: "2026-01-01".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };
        let collision = crate::db_core::models::Image {
            id: "collision".to_string(),
            sha256_hash: "collision-hash".to_string(),
            ..inside.clone()
        };
        db.insert_image(&inside).unwrap();
        db.insert_image(&collision).unwrap();
        db.insert_image_file(&crate::db_core::models::ImageFile {
            id: "f-inside".to_string(),
            image_id: "inside".to_string(),
            path: source_file.to_string_lossy().to_string(),
            last_seen_at: "2026-01-01".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
        db.insert_image_file(&crate::db_core::models::ImageFile {
            id: "f-collision".to_string(),
            image_id: "collision".to_string(),
            path: tmp
                .path()
                .join("renamed/image.png")
                .to_string_lossy()
                .to_string(),
            last_seen_at: "2026-01-01".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
        db.add_library_root(&source.to_string_lossy()).unwrap();
        let watcher = parking_lot::Mutex::new(crate::watcher::FileWatcher::new());

        let error = rename_folder_on_disk_and_db(&db, &watcher, &source, "renamed").unwrap_err();

        assert!(error.contains("target subtree"));
        assert!(source.exists());
        assert!(source_file.exists());
        assert!(!tmp.path().join("renamed").exists());
        assert!(db
            .get_image_file_by_path(&source_file.to_string_lossy())
            .unwrap()
            .is_some());
    }

    #[test]
    fn pending_folder_rename_recovery_restores_database_when_disk_did_not_move() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        std::fs::create_dir(&source).unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.add_library_root(&target.to_string_lossy()).unwrap();
        db.set_setting(
            PENDING_FOLDER_RENAME_SETTING,
            &serde_json::to_string(&PendingFolderRename {
                source: source.to_string_lossy().to_string(),
                target: target.to_string_lossy().to_string(),
            })
            .unwrap(),
        )
        .unwrap();

        recover_pending_folder_rename(&db).unwrap();

        assert_eq!(
            db.list_library_roots().unwrap(),
            vec![source.to_string_lossy()]
        );
        assert!(db
            .get_setting(PENDING_FOLDER_RENAME_SETTING)
            .unwrap()
            .is_none());
    }

    #[test]
    fn pending_folder_rename_recovery_accepts_committed_database_when_disk_moved() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.add_library_root(&target.to_string_lossy()).unwrap();
        db.set_setting(
            PENDING_FOLDER_RENAME_SETTING,
            &serde_json::to_string(&PendingFolderRename {
                source: source.to_string_lossy().to_string(),
                target: target.to_string_lossy().to_string(),
            })
            .unwrap(),
        )
        .unwrap();

        recover_pending_folder_rename(&db).unwrap();

        assert_eq!(
            db.list_library_roots().unwrap(),
            vec![target.to_string_lossy()]
        );
        assert!(db
            .get_setting(PENDING_FOLDER_RENAME_SETTING)
            .unwrap()
            .is_none());
    }

    #[test]
    fn exclusive_directory_rename_never_replaces_an_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(source.join("source.txt"), b"source").unwrap();
        std::fs::write(target.join("unrelated.txt"), b"unrelated").unwrap();

        assert!(rename_directory_exclusive(&source, &target).is_err());
        assert_eq!(std::fs::read(source.join("source.txt")).unwrap(), b"source");
        assert_eq!(
            std::fs::read(target.join("unrelated.txt")).unwrap(),
            b"unrelated"
        );
    }

    #[test]
    fn watcher_registration_failure_removes_every_partial_new_root() {
        let roots = vec!["/new/one".to_string(), "/new/two".to_string()];
        let mut watcher = FailingWatchOps {
            fail_on: "/new/two".to_string(),
            ..Default::default()
        };

        let error = register_new_watcher_roots(&mut watcher, &roots).unwrap_err();

        assert!(error.contains("cannot watch /new/two"));
        assert_eq!(watcher.added, vec!["/new/one"]);
        assert_eq!(watcher.removed, vec!["/new/one"]);
    }

    #[test]
    fn stale_recovery_journal_is_never_overwritten_by_a_new_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.add_library_root(&source.to_string_lossy()).unwrap();
        let stale = r#"{"source":"/old/source","target":"/old/target"}"#;
        db.set_setting(PENDING_FOLDER_RENAME_SETTING, stale)
            .unwrap();
        let watcher = parking_lot::Mutex::new(crate::watcher::FileWatcher::new());

        let error = rename_folder_on_disk_and_db(&db, &watcher, &source, "renamed").unwrap_err();

        assert!(error.contains("requires recovery"));
        assert_eq!(
            db.get_setting(PENDING_FOLDER_RENAME_SETTING)
                .unwrap()
                .as_deref(),
            Some(stale)
        );
        assert!(source.exists());
        assert!(!tmp.path().join("renamed").exists());
    }

    #[test]
    fn rollback_keeps_forward_database_paths_when_disk_cannot_be_restored() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(source.join("unrelated.txt"), b"unrelated").unwrap();
        std::fs::write(target.join("moved.txt"), b"moved").unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.add_library_root(&target.to_string_lossy()).unwrap();
        db.set_setting(PENDING_FOLDER_RENAME_SETTING, "pending")
            .unwrap();
        let watcher = parking_lot::Mutex::new(crate::watcher::FileWatcher::new());

        let error = rollback_folder_rename(
            &db,
            &watcher,
            &source,
            &target,
            &[source.to_string_lossy().to_string()],
            &[target.to_string_lossy().to_string()],
            true,
        )
        .unwrap_err();

        assert!(error.contains("failed to restore folder on disk"));
        assert_eq!(
            db.list_library_roots().unwrap(),
            vec![target.to_string_lossy()]
        );
        assert!(db
            .get_setting(PENDING_FOLDER_RENAME_SETTING)
            .unwrap()
            .is_some());
        assert_eq!(
            std::fs::read(source.join("unrelated.txt")).unwrap(),
            b"unrelated"
        );
        assert_eq!(std::fs::read(target.join("moved.txt")).unwrap(), b"moved");
    }

    #[test]
    fn rename_allows_a_parent_containing_only_managed_library_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let group = tmp.path().join("group");
        let first = group.join("first");
        let second = group.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.add_library_root(&first.to_string_lossy()).unwrap();
        db.add_library_root(&second.to_string_lossy()).unwrap();
        let watcher = parking_lot::Mutex::new(crate::watcher::FileWatcher::new());

        rename_folder_on_disk_and_db(&db, &watcher, &group, "renamed").unwrap();

        assert!(!group.exists());
        let renamed = tmp.path().join("renamed");
        assert!(renamed.join("first").exists());
        assert!(renamed.join("second").exists());
        assert_eq!(
            db.list_library_roots().unwrap(),
            vec![
                renamed.join("first").to_string_lossy().to_string(),
                renamed.join("second").to_string_lossy().to_string(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn rename_folder_rejects_symlink_escape_from_library_root() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("library");
        let external = tmp.path().join("external");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&external).unwrap();
        let escaped = root.join("linked-folder");
        symlink(&external, &escaped).unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.add_library_root(&root.to_string_lossy()).unwrap();
        let watcher = parking_lot::Mutex::new(crate::watcher::FileWatcher::new());

        let error = rename_folder_on_disk_and_db(&db, &watcher, &escaped, "renamed").unwrap_err();

        assert!(error.contains("symbolic link"));
        assert!(external.exists());
        assert!(!root.join("renamed").exists());
    }

    #[test]
    fn validate_app_bundle_accepts_system_app() {
        // /Applications/Preview.app should exist on any macOS system
        let app = Path::new("/Applications/Preview.app");
        if app.exists() {
            assert!(validate_app_bundle(app).is_ok());
        }
    }

    #[test]
    fn validate_app_bundle_rejects_non_app_directory() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Preview");
        std::fs::create_dir(&app).unwrap();

        assert_eq!(
            validate_app_bundle(&app).unwrap_err(),
            "Choose a macOS .app bundle"
        );
    }

    #[test]
    fn validate_app_bundle_rejects_app_outside_allowed_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Evil.app");
        std::fs::create_dir(&app).unwrap();

        let err = validate_app_bundle(&app).unwrap_err();
        assert!(
            err.contains("outside allowed directories"),
            "Expected allowlist error, got: {}",
            err
        );
    }

    #[test]
    fn validate_app_bundle_rejects_missing_app_extension() {
        let err = validate_app_bundle(Path::new("/Applications/SomeApp")).unwrap_err();
        assert_eq!(err, "Choose a macOS .app bundle");
    }

    #[test]
    fn app_display_name_uses_bundle_stem() {
        assert_eq!(
            app_display_name(Path::new("/Applications/Preview.app")),
            "Preview"
        );
    }

    #[test]
    fn paste_filename_continues_folder_wide_numeric_sequence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file_01.png"), b"one").unwrap();
        std::fs::write(dir.path().join("file_02.png"), b"two").unwrap();

        let name =
            next_paste_filename(dir.path(), "png", Some("ignored.png"), "2026-06-02").unwrap();

        assert_eq!(name, "file_03.png");
    }

    #[test]
    fn paste_filename_uses_configured_date_prefix_without_sequence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("portrait.png"), b"existing").unwrap();

        let name =
            next_paste_filename(dir.path(), "png", Some("Source Image.png"), "2026.06.02").unwrap();

        assert_eq!(name, "2026.06.02-source-image.png");
    }

    #[test]
    fn paste_filename_adds_counter_for_date_prefix_collisions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2026-06-02-source-image.png"), b"existing").unwrap();

        let name =
            next_paste_filename(dir.path(), "png", Some("Source Image.png"), "2026-06-02").unwrap();

        assert_eq!(name, "2026-06-02-source-image-02.png");
    }
}
