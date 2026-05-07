use image::imageops::FilterType;
use std::path::{Path, PathBuf};

const THUMBNAIL_SIZE: u32 = 400;

pub fn thumbnail_dir(app_data_dir: &Path) -> PathBuf {
    let dir = app_data_dir.join("thumbnails");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn generate_thumbnail(
    source_path: &Path,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<PathBuf, String> {
    let img = image::open(source_path).map_err(|e| format!("Failed to open image: {}", e))?;
    let thumb = img.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Lanczos3);
    let thumb_dir = thumbnail_dir(app_data_dir);
    let thumb_path = thumb_dir.join(format!("{}.jpg", image_id));
    thumb.save(&thumb_path).map_err(|e| format!("Failed to save thumbnail: {}", e))?;
    Ok(thumb_path)
}

pub fn thumbnail_path(app_data_dir: &Path, image_id: &str) -> PathBuf {
    thumbnail_dir(app_data_dir).join(format!("{}.jpg", image_id))
}
