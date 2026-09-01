use crate::db_core::db::Database;
use crate::db_core::import::sync_referenced_file_cancellable;
use crate::db_core::models::ReferencedSource;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenReferencedFolder {
    pub source_id: String,
    #[serde(default)]
    pub relative_path: String,
    #[serde(default)]
    pub recursive: bool,
    pub cursor: Option<String>,
    #[serde(default = "default_page_limit")]
    pub limit: u32,
}

fn default_page_limit() -> u32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferencedFolderPage {
    pub job_id: String,
    pub source_id: String,
    pub relative_path: String,
    pub requested_paths: Vec<String>,
    pub image_ids: Vec<String>,
    pub discovered_count: u32,
    pub next_cursor: Option<String>,
    pub indexing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferencedFolderUpdate {
    pub job_id: String,
    pub source_id: String,
    pub relative_path: String,
    pub image_ids: Vec<String>,
    pub completed: bool,
    pub cancelled: bool,
    pub error: Option<String>,
}

fn source_by_id(db: &Database, source_id: &str) -> Result<ReferencedSource, String> {
    db.list_referenced_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| "Referenced source not found".to_string())
}

fn safe_relative_path(relative_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(relative_path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("Folder path must remain inside the referenced source".to_string());
    }
    Ok(path)
}

fn mounted_root(source: &ReferencedSource) -> Result<PathBuf, String> {
    if source.offline_at.is_some() {
        return Err(format!(
            "Reconnect {} to browse originals",
            source.display_name
        ));
    }
    let root = source
        .last_mount_path
        .as_deref()
        .ok_or_else(|| "Referenced source has no mounted path".to_string())?;
    std::fs::canonicalize(root)
        .map_err(|_| format!("Reconnect {} to browse originals", source.display_name))
}

fn relative_string(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
}

pub fn list_source_folders(
    db: &Database,
    source_id: &str,
    relative_path: &str,
) -> Result<Vec<String>, String> {
    let source = source_by_id(db, source_id)?;
    let root = mounted_root(&source)?;
    let folder = root.join(safe_relative_path(relative_path)?);
    let folder = std::fs::canonicalize(&folder).map_err(|error| error.to_string())?;
    if !folder.starts_with(&root) {
        return Err("Folder path must remain inside the referenced source".to_string());
    }
    let mut folders = std::fs::read_dir(folder)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            let path = entry.path().canonicalize().ok()?;
            if !path.starts_with(&root) {
                return None;
            }
            relative_string(&root, &path)
        })
        .collect::<Vec<_>>();
    folders.sort_by_key(|path| path.to_lowercase());
    Ok(folders)
}

pub fn discover_folder_page(
    db: &Database,
    request: &OpenReferencedFolder,
) -> Result<(ReferencedFolderPage, Vec<PathBuf>), String> {
    let source = source_by_id(db, &request.source_id)?;
    let root = mounted_root(&source)?;
    let requested_relative = safe_relative_path(&request.relative_path)?;
    let folder =
        std::fs::canonicalize(root.join(&requested_relative)).map_err(|error| error.to_string())?;
    if !folder.starts_with(&root) || !folder.is_dir() {
        return Err("Folder path must remain inside the referenced source".to_string());
    }
    let module_raw = crate::db_core::import::is_module_raw_enabled(db);
    let limit = request.limit.clamp(1, 250) as usize;
    let cursor = request.cursor.as_deref();
    let mut candidates = Vec::with_capacity(limit + 1);

    if request.recursive {
        for entry in WalkDir::new(&folder)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file()
                || !crate::extensions::is_image_path(entry.path(), module_raw)
            {
                continue;
            }
            let Ok(canonical) = entry.path().canonicalize() else {
                continue;
            };
            if !canonical.starts_with(&root) {
                continue;
            }
            let Some(relative) = relative_string(&root, &canonical) else {
                continue;
            };
            if cursor.is_some_and(|cursor| relative.as_str() <= cursor) {
                continue;
            }
            candidates.push((relative, canonical));
            if candidates.len() > limit {
                break;
            }
        }
    } else {
        let mut direct = std::fs::read_dir(&folder)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_file() || !crate::extensions::is_image_path(&path, module_raw) {
                    return None;
                }
                let canonical = path.canonicalize().ok()?;
                if !canonical.starts_with(&root) {
                    return None;
                }
                Some((relative_string(&root, &canonical)?, canonical))
            })
            .collect::<Vec<_>>();
        direct.sort_by(|a, b| a.0.cmp(&b.0));
        candidates.extend(
            direct
                .into_iter()
                .filter(|(relative, _)| cursor.is_none_or(|cursor| relative.as_str() > cursor))
                .take(limit + 1),
        );
    }

    let has_more = candidates.len() > limit;
    candidates.truncate(limit);
    let next_cursor = has_more
        .then(|| candidates.last().map(|(relative, _)| relative.clone()))
        .flatten();
    let requested_paths = candidates
        .iter()
        .map(|(relative, _)| relative.clone())
        .collect::<Vec<_>>();
    let paths = candidates.into_iter().map(|(_, path)| path).collect();
    Ok((
        ReferencedFolderPage {
            job_id: uuid::Uuid::new_v4().to_string(),
            source_id: request.source_id.clone(),
            relative_path: request.relative_path.clone(),
            discovered_count: requested_paths.len() as u32,
            requested_paths,
            image_ids: Vec::new(),
            next_cursor,
            indexing: true,
        },
        paths,
    ))
}

pub fn register_referenced_paths(
    db: &Database,
    app_data_dir: &Path,
    source_id: &str,
    paths: &[PathBuf],
    cancelled: &Arc<AtomicBool>,
) -> Result<Vec<String>, String> {
    let source = source_by_id(db, source_id)?;
    let root = mounted_root(&source)?;
    let mut image_ids = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        sync_referenced_file_cancellable(db, path, app_data_dir, &|| {
            cancelled.load(Ordering::Relaxed)
        })?;
        let path_string = path.to_string_lossy();
        let Some(file) = db
            .get_image_file_by_path(&path_string)
            .map_err(|error| error.to_string())?
        else {
            continue;
        };
        let thumbnail = crate::db_core::thumbnails::thumbnail_path(app_data_dir, &file.image_id);
        if !thumbnail.exists() {
            crate::db_core::thumbnails::generate_thumbnail(path, app_data_dir, &file.image_id)
                .map_err(|error| format!("Referenced indexing thumbnail failed: {error}"))?;
        }
        let relative = relative_string(&root, path)
            .ok_or_else(|| "Indexed file escaped referenced source".to_string())?;
        db.attach_referenced_file(source_id, &file.id, &relative)
            .map_err(|error| error.to_string())?;
        if seen.insert(file.image_id.clone()) {
            image_ids.push(file.image_id);
        }
    }
    Ok(image_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::{ReferencedSource, ReferencedSourceKind};
    use std::fs;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, Database, ReferencedSource) {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("cull.db")).unwrap();
        let source = ReferencedSource {
            id: "source".into(),
            platform_volume_id: Some("volume".into()),
            display_name: "CARD".into(),
            last_mount_path: Some(dir.path().to_string_lossy().to_string()),
            source_kind: ReferencedSourceKind::SdCard,
            capacity_bytes: None,
            recursive_default: false,
            settings_json: "{}".into(),
            last_seen_at: "2026-08-30".into(),
            offline_at: None,
        };
        db.upsert_referenced_source(&source).unwrap();
        (dir, db, source)
    }

    #[test]
    fn discovery_is_paged_sorted_and_non_recursive_by_default() {
        let (dir, db, source) = setup();
        let dcim = dir.path().join("DCIM");
        fs::create_dir_all(dcim.join("nested")).unwrap();
        for index in (0..55).rev() {
            fs::write(dcim.join(format!("IMG_{index:04}.JPG")), []).unwrap();
        }
        fs::write(dcim.join("notes.txt"), []).unwrap();
        fs::write(dcim.join("nested/hidden.jpg"), []).unwrap();
        let request = OpenReferencedFolder {
            source_id: source.id,
            relative_path: "DCIM".into(),
            recursive: false,
            cursor: None,
            limit: 50,
        };
        let (page, _) = discover_folder_page(&db, &request).unwrap();
        assert_eq!(page.requested_paths.len(), 50);
        assert!(page.next_cursor.is_some());
        assert!(page
            .requested_paths
            .iter()
            .all(|path| !path.contains("nested")));
        assert_eq!(page.requested_paths[0], "DCIM/IMG_0000.JPG");
    }

    #[test]
    fn recursive_discovery_includes_nested_images_and_rejects_escape() {
        let (dir, db, source) = setup();
        fs::create_dir_all(dir.path().join("DCIM/nested")).unwrap();
        fs::write(dir.path().join("DCIM/nested/image.jpg"), []).unwrap();
        let request = OpenReferencedFolder {
            source_id: source.id.clone(),
            relative_path: "DCIM".into(),
            recursive: true,
            cursor: None,
            limit: 50,
        };
        let (page, _) = discover_folder_page(&db, &request).unwrap();
        assert_eq!(page.requested_paths, vec!["DCIM/nested/image.jpg"]);
        assert!(discover_folder_page(
            &db,
            &OpenReferencedFolder {
                relative_path: "../".into(),
                ..request
            }
        )
        .is_err());
    }

    #[test]
    fn unchanged_referenced_file_regenerates_a_purged_thumbnail() {
        let (dir, db, source) = setup();
        let image_path = dir.path().join("image.jpg");
        let app_data_dir = dir.path().join("app-data");
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([90, 120, 150]));
        image.save(&image_path).unwrap();
        let image_path = fs::canonicalize(image_path).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));

        let image_ids = register_referenced_paths(
            &db,
            &app_data_dir,
            &source.id,
            std::slice::from_ref(&image_path),
            &cancelled,
        )
        .unwrap();
        let image_id = image_ids.first().unwrap();
        let thumbnail = crate::db_core::thumbnails::thumbnail_path(&app_data_dir, image_id);
        assert!(thumbnail.exists());

        crate::db_core::thumbnails::remove_thumbnails_for_image(&app_data_dir, image_id);
        assert!(!thumbnail.exists());

        register_referenced_paths(
            &db,
            &app_data_dir,
            &source.id,
            std::slice::from_ref(&image_path),
            &cancelled,
        )
        .unwrap();

        assert!(thumbnail.exists());
    }

    #[test]
    fn browsing_then_importing_the_same_path_promotes_it_without_changing_image_identity() {
        let (dir, db, source) = setup();
        let image_path = dir.path().join("promote.jpg");
        let app_data_dir = dir.path().join("app-data");
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([90, 120, 150]));
        image.save(&image_path).unwrap();
        let image_path = fs::canonicalize(image_path).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));

        let browsed_ids = register_referenced_paths(
            &db,
            &app_data_dir,
            &source.id,
            std::slice::from_ref(&image_path),
            &cancelled,
        )
        .unwrap();
        assert_eq!(browsed_ids.len(), 1);
        assert!(db.list_images(20, 0).unwrap().is_empty());

        let imported_id = crate::db_core::import::import_file(&db, &image_path, &app_data_dir)
            .unwrap()
            .expect("promoting a browsed path must count as an import");

        assert_eq!(imported_id, browsed_ids[0]);
        let library = db.list_images(20, 0).unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].image.id, browsed_ids[0]);
        let referenced = db
            .list_images_in_referenced_folder(&source.id, "", true, 20, 0, true)
            .unwrap();
        assert_eq!(referenced.len(), 1);
        assert_eq!(referenced[0].image.id, browsed_ids[0]);
    }

    #[test]
    fn importing_then_browsing_the_same_path_keeps_it_in_the_library() {
        let (dir, db, source) = setup();
        let image_path = dir.path().join("keep.jpg");
        let app_data_dir = dir.path().join("app-data");
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([60, 80, 100]));
        image.save(&image_path).unwrap();
        let image_path = fs::canonicalize(image_path).unwrap();

        let imported_id = crate::db_core::import::import_file(&db, &image_path, &app_data_dir)
            .unwrap()
            .unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let browsed_ids = register_referenced_paths(
            &db,
            &app_data_dir,
            &source.id,
            std::slice::from_ref(&image_path),
            &cancelled,
        )
        .unwrap();

        assert_eq!(browsed_ids, vec![imported_id.clone()]);
        let library = db.list_images(20, 0).unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].image.id, imported_id);
    }

    #[test]
    fn a_failed_explicit_import_does_not_promote_a_browsed_path() {
        let (dir, db, source) = setup();
        let image_path = dir.path().join("broken-on-import.jpg");
        let app_data_dir = dir.path().join("app-data");
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([40, 50, 60]));
        image.save(&image_path).unwrap();
        let image_path = fs::canonicalize(image_path).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        register_referenced_paths(
            &db,
            &app_data_dir,
            &source.id,
            std::slice::from_ref(&image_path),
            &cancelled,
        )
        .unwrap();
        assert!(db.list_images(20, 0).unwrap().is_empty());

        fs::write(&image_path, b"not a decodable jpeg anymore").unwrap();
        assert!(crate::db_core::import::import_file(&db, &image_path, &app_data_dir).is_err());

        assert!(db.list_images(20, 0).unwrap().is_empty());
    }
}
