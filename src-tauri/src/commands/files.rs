use crate::AppState;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

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
            eprintln!(
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
}
