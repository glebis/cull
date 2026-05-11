use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use dashmap::DashMap;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use tauri::{AppHandle, Emitter};
use crate::db_core::db::Database;

pub struct MoveIntent {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
    pub image_file_id: String,
    pub registered_at: Instant,
}

pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    intent_registry: Arc<DashMap<PathBuf, MoveIntent>>,
}

const INTENT_EXPIRY_SECS: u64 = 60;
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif",
    "heic", "heif", "avif", "svg", "ico",
    "cr2", "cr3", "nef", "arw", "dng", "orf", "raf", "rw2", "psd",
];

/// Try to find an image_file record by path, attempting both the given path
/// and common macOS symlink variants (/tmp ↔ /private/tmp).
fn find_file_by_path_flexible(db: &Database, path: &std::path::Path) -> Option<crate::db_core::models::ImageFile> {
    let path_str = path.to_string_lossy();
    if let Ok(Some(f)) = db.get_image_file_by_path(&path_str) {
        return Some(f);
    }
    // macOS: /tmp is a symlink to /private/tmp — try the alternate form
    let alt = if path_str.starts_with("/private/") {
        path_str.replacen("/private", "", 1)
    } else {
        format!("/private{}", path_str)
    };
    db.get_image_file_by_path(&alt).ok().flatten()
}

fn normalize_path_for_db(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    // Strip /private prefix if the DB likely stores without it
    if s.starts_with("/private/tmp") {
        s.replacen("/private", "", 1)
    } else {
        s.to_string()
    }
}

fn is_image_ext(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            intent_registry: Arc::new(DashMap::new()),
        }
    }

    pub fn intent_registry(&self) -> Arc<DashMap<PathBuf, MoveIntent>> {
        self.intent_registry.clone()
    }

    pub fn register_move_intent(&self, old_path: PathBuf, new_path: PathBuf, image_file_id: String) {
        let registered_at = Instant::now();
        self.intent_registry.insert(old_path.clone(), MoveIntent {
            old_path: old_path.clone(),
            new_path: new_path.clone(),
            image_file_id: image_file_id.clone(),
            registered_at,
        });
        self.intent_registry.insert(new_path.clone(), MoveIntent {
            old_path,
            new_path,
            image_file_id,
            registered_at,
        });
    }

    pub fn start(&mut self, db: Database, app_handle: AppHandle, roots: Vec<String>) -> Result<(), String> {
        eprintln!("[watcher] Starting with {} roots", roots.len());
        let db = Arc::new(db);
        let intent_reg = self.intent_registry.clone();
        let handle = app_handle.clone();

        let db_clone = db.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    Self::handle_event(event, &db_clone, &handle, &intent_reg);
                }
                Err(e) => {
                    eprintln!("[watcher] Error: {}", e);
                }
            }
        }).map_err(|e| format!("Failed to create watcher: {}", e))?;

        for root in &roots {
            let path = PathBuf::from(root);
            if path.exists() {
                match watcher.watch(&path, RecursiveMode::Recursive) {
                    Ok(()) => eprintln!("[watcher] Watching: {}", root),
                    Err(e) => eprintln!("[watcher] Failed to watch {}: {}", root, e),
                }
            } else {
                eprintln!("[watcher] Root does not exist, skipping: {}", root);
            }
        }

        self.watcher = Some(watcher);
        eprintln!("[watcher] Started successfully");
        Ok(())
    }

    pub fn watch_folder(&mut self, path: &str) -> Result<(), String> {
        if let Some(ref mut w) = self.watcher {
            let p = PathBuf::from(path);
            if p.exists() {
                w.watch(&p, RecursiveMode::Recursive)
                    .map_err(|e| format!("Failed to watch {}: {}", path, e))?;
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn unwatch_folder(&mut self, path: &str) -> Result<(), String> {
        if let Some(ref mut w) = self.watcher {
            let p = PathBuf::from(path);
            w.unwatch(&p)
                .map_err(|e| format!("Failed to unwatch {}: {}", path, e))?;
        }
        Ok(())
    }

    fn handle_event(
        event: Event,
        db: &Database,
        app_handle: &AppHandle,
        intent_registry: &DashMap<PathBuf, MoveIntent>,
    ) {
        let now = Instant::now();
        intent_registry.retain(|_, intent| {
            now.duration_since(intent.registered_at).as_secs() < INTENT_EXPIRY_SECS
        });

        let mut changed = false;

        match event.kind {
            EventKind::Remove(_) => {
                for path in &event.paths {
                    if !is_image_ext(path) { continue; }
                    if intent_registry.remove(path).is_some() {
                        continue;
                    }
                    // Use flexible lookup to handle macOS symlink paths
                    if let Some(file) = find_file_by_path_flexible(db, path) {
                        if file.missing_at.is_none() {
                            match db.mark_file_missing(&file.path) {
                                Ok(true) => {
                                    eprintln!("[watcher] Marked missing: {}", file.path);
                                    changed = true;
                                }
                                Ok(false) => {}
                                Err(e) => eprintln!("[watcher] Error marking missing: {}", e),
                            }
                        }
                    }
                }
            }
            EventKind::Create(_) => {
                for path in &event.paths {
                    if !is_image_ext(path) { continue; }
                    if intent_registry.remove(path).is_some() {
                        continue;
                    }
                    // Use flexible lookup to handle macOS symlink paths
                    match find_file_by_path_flexible(db, path) {
                        Some(file) if file.missing_at.is_some() => {
                            match db.restore_file(&file.path) {
                                Ok(true) => {
                                    eprintln!("[watcher] Restored: {}", file.path);
                                    changed = true;
                                }
                                Ok(false) => {}
                                Err(e) => eprintln!("[watcher] Error restoring: {}", e),
                            }
                        }
                        Some(_) => {}
                        None => {
                            let path_str = path.to_string_lossy();
                            eprintln!("[watcher] New file detected: {}", path_str);
                            let _ = app_handle.emit("watcher:new-file", path_str.to_string());
                            changed = true;
                        }
                    }
                }
            }
            EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                if event.paths.len() == 2 {
                    let old = &event.paths[0];
                    let new = &event.paths[1];
                    if is_image_ext(old) || is_image_ext(new) {
                        if intent_registry.remove(old).is_some() || intent_registry.remove(new).is_some() {
                            return;
                        }
                        let new_str = normalize_path_for_db(new);
                        match find_file_by_path_flexible(db, old) {
                            Some(file) => {
                                match db.update_image_file_path(&file.id, &new_str) {
                                    Ok(()) => {
                                        eprintln!("[watcher] Renamed: {} -> {}", file.path, new_str);
                                        changed = true;
                                    }
                                    Err(e) => eprintln!("[watcher] Error updating path: {}", e),
                                }
                            }
                            None => {}
                        }
                    }
                } else {
                    for path in &event.paths {
                        if !is_image_ext(path) { continue; }
                        if intent_registry.remove(path).is_some() {
                            continue;
                        }
                        if !path.exists() {
                            if let Some(file) = find_file_by_path_flexible(db, path) {
                                let _ = db.mark_file_missing(&file.path);
                                changed = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        if changed {
            let _ = app_handle.emit("images:changed", ());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_new_creates_empty_watcher() {
        let fw = FileWatcher::new();
        assert!(fw.intent_registry.is_empty());
    }

    #[test]
    fn test_register_move_intent_adds_both_paths() {
        let fw = FileWatcher::new();
        let old = PathBuf::from("/photos/a.png");
        let new = PathBuf::from("/archive/a.png");
        fw.register_move_intent(old.clone(), new.clone(), "file-1".to_string());

        let reg = &fw.intent_registry;
        assert_eq!(reg.len(), 2);
        assert!(reg.contains_key(&old));
        assert!(reg.contains_key(&new));
    }

    #[test]
    fn test_intent_registry_remove_clears_entry() {
        let fw = FileWatcher::new();
        let old = PathBuf::from("/photos/a.png");
        let new = PathBuf::from("/archive/a.png");
        fw.register_move_intent(old.clone(), new.clone(), "file-1".to_string());

        let reg = &fw.intent_registry;
        assert!(reg.remove(&old).is_some());
        assert!(reg.remove(&old).is_none());
        assert!(reg.remove(&new).is_some());
        assert!(reg.is_empty());
    }

    #[test]
    fn test_intent_registry_unknown_path_returns_none() {
        let fw = FileWatcher::new();
        let unknown = PathBuf::from("/unknown/path.png");
        assert!(fw.intent_registry.clone().remove(&unknown).is_none());
    }

    #[test]
    fn test_multiple_intents_independent() {
        let fw = FileWatcher::new();
        fw.register_move_intent(
            PathBuf::from("/a.png"),
            PathBuf::from("/b.png"),
            "f1".to_string(),
        );
        fw.register_move_intent(
            PathBuf::from("/c.png"),
            PathBuf::from("/d.png"),
            "f2".to_string(),
        );

        let reg = &fw.intent_registry;
        assert_eq!(reg.len(), 4);
        reg.remove(&PathBuf::from("/a.png"));
        assert_eq!(reg.len(), 3);
        assert!(reg.contains_key(&PathBuf::from("/b.png")));
        assert!(reg.contains_key(&PathBuf::from("/c.png")));
        assert!(reg.contains_key(&PathBuf::from("/d.png")));
    }

    #[test]
    fn test_overwrite_intent_same_path() {
        let fw = FileWatcher::new();
        fw.register_move_intent(
            PathBuf::from("/a.png"),
            PathBuf::from("/b.png"),
            "f1".to_string(),
        );
        fw.register_move_intent(
            PathBuf::from("/a.png"),
            PathBuf::from("/c.png"),
            "f2".to_string(),
        );

        let reg = &fw.intent_registry;
        // /a.png overwritten, /b.png from first, /a.png + /c.png from second
        assert_eq!(reg.len(), 3);
        assert!(reg.contains_key(&PathBuf::from("/a.png")));
        assert!(reg.contains_key(&PathBuf::from("/b.png")));
        assert!(reg.contains_key(&PathBuf::from("/c.png")));
    }

    #[test]
    fn test_is_image_ext_recognizes_common_formats() {
        assert!(is_image_ext(std::path::Path::new("photo.jpg")));
        assert!(is_image_ext(std::path::Path::new("photo.JPEG")));
        assert!(is_image_ext(std::path::Path::new("photo.png")));
        assert!(is_image_ext(std::path::Path::new("photo.webp")));
        assert!(is_image_ext(std::path::Path::new("photo.cr2")));
        assert!(is_image_ext(std::path::Path::new("photo.dng")));
        assert!(is_image_ext(std::path::Path::new("photo.heic")));
        assert!(is_image_ext(std::path::Path::new("photo.psd")));
    }

    #[test]
    fn test_is_image_ext_rejects_non_images() {
        assert!(!is_image_ext(std::path::Path::new("doc.txt")));
        assert!(!is_image_ext(std::path::Path::new("data.json")));
        assert!(!is_image_ext(std::path::Path::new("script.rs")));
        assert!(!is_image_ext(std::path::Path::new("noext")));
        assert!(!is_image_ext(std::path::Path::new(".hidden")));
    }
}
