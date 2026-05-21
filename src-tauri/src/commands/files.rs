use crate::AppState;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State, Window};

#[derive(Debug, Clone, Serialize)]
pub struct OpenWithApplication {
    name: String,
    path: String,
    is_default: bool,
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
    if !app_path.exists() {
        return Err(format!("Application not found: {}", app_path.display()));
    }
    if !app_path.is_dir() {
        return Err("Choose a macOS .app bundle".to_string());
    }
    let is_app_bundle = app_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("app"))
        .unwrap_or(false);
    if !is_app_bundle {
        return Err("Choose a macOS .app bundle".to_string());
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
    fn validate_app_bundle_accepts_app_directory() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Preview.app");
        std::fs::create_dir(&app).unwrap();

        assert!(validate_app_bundle(&app).is_ok());
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
    fn app_display_name_uses_bundle_stem() {
        assert_eq!(
            app_display_name(Path::new("/Applications/Preview.app")),
            "Preview"
        );
    }
}
