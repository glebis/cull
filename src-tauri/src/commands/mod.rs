pub mod collections;
pub mod deeplink;
pub mod detection;
pub mod raw;
pub mod dictation;
pub mod embeddings;
pub mod export;
pub mod files;
pub mod generation;
pub mod import;
pub mod jobs;
pub mod library;
pub mod lineage;
pub mod mcp;
pub mod selection;
pub mod smart_collections;
pub mod vision;
pub mod transform;
pub mod undo;
pub mod window;
pub mod sessions;
pub mod privacy;

pub fn resolve_image_path_for_ml(img: &crate::db_core::models::ImageWithFile, app_data_dir: &std::path::Path) -> std::path::PathBuf {
    let ext = std::path::Path::new(&img.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if crate::extensions::is_raw_extension(ext) {
        crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id)
    } else {
        std::path::PathBuf::from(&img.path)
    }
}
