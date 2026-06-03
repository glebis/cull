use crate::AppState;
use serde::Serialize;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State, Window};

const CLIPBOARD_PASTE_DATE_FORMAT_SETTING: &str = "clipboard_paste_date_format";
const DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT: &str = "%Y-%m-%d";

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

    Ok(new_path_str)
}

#[tauri::command]
pub async fn rename_image(
    app: AppHandle,
    state: State<'_, AppState>,
    image_id: String,
    new_name: String,
) -> Result<String, String> {
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

    Ok(new_folder.to_string_lossy().to_string())
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

    let id_refs: Vec<&str> = image_ids.iter().map(|id| id.as_str()).collect();
    let found = state
        .db
        .get_images_by_ids(&id_refs)
        .map_err(|e| e.to_string())?;
    let img = found
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_ids[0]))?;

    let path = PathBuf::from(&img.path);
    if !path.exists() {
        return Err(format!("Cannot open missing file: {}", img.path));
    }

    open_paths_with_application(&app_bundle, vec![path])
}

#[tauri::command]
pub async fn list_open_with_applications(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Vec<OpenWithApplication>, String> {
    let images = state
        .db
        .get_images_by_ids(&[&image_id])
        .map_err(|e| e.to_string())?;
    let img = images
        .first()
        .ok_or_else(|| format!("Image '{}' not found", image_id))?;

    let path = PathBuf::from(&img.path);
    if !path.exists() {
        return Err(format!(
            "Cannot list applications for missing file: {}",
            img.path
        ));
    }

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
    use std::io::Write;

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
