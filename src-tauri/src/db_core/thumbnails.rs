use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const JPEG_QUALITY: u8 = 90;
pub const THUMBNAIL_SIZES: [u32; 4] = [64, 128, 256, 800];

pub fn thumbnail_dir(app_data_dir: &Path) -> PathBuf {
    let dir = app_data_dir.join("thumbnails");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn save_jpeg(img: &image::DynamicImage, path: &Path) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("Failed to create thumbnail file: {}", e))?;
    let writer = BufWriter::new(file);
    let encoder = JpegEncoder::new_with_quality(writer, JPEG_QUALITY);
    img.write_with_encoder(encoder)
        .map_err(|e| format!("Failed to save thumbnail: {}", e))?;
    Ok(())
}

pub fn generate_thumbnail(
    source_path: &Path,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<PathBuf, String> {
    let img = image::open(source_path).map_err(|e| format!("Failed to open image: {}", e))?;
    let thumb_dir = thumbnail_dir(app_data_dir);

    let mut current = img;
    let src_max = current.width().max(current.height());
    let last_path = thumb_dir.join(format!("{}.jpg", image_id));

    for &size in THUMBNAIL_SIZES.iter().rev() {
        if size >= src_max {
            // Never upscale — save a copy at native resolution instead
            if size == 800 {
                save_jpeg(&current, &last_path)?;
            } else {
                let sized_path = thumb_dir.join(format!("{}_{}.jpg", image_id, size));
                save_jpeg(&current, &sized_path)?;
            }
            continue;
        }
        let resized = current.resize(size, size, FilterType::Lanczos3);
        if size == 800 {
            save_jpeg(&resized, &last_path)?;
        } else {
            let sized_path = thumb_dir.join(format!("{}_{}.jpg", image_id, size));
            save_jpeg(&resized, &sized_path)?;
        }
        current = resized;
    }

    Ok(last_path)
}

pub fn generate_thumbnail_from_image(
    img: &image::DynamicImage,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<PathBuf, String> {
    let thumb_dir = thumbnail_dir(app_data_dir);
    let mut current = img.clone();
    let src_max = current.width().max(current.height());
    let last_path = thumb_dir.join(format!("{}.jpg", image_id));

    for &size in THUMBNAIL_SIZES.iter().rev() {
        if size >= src_max {
            if size == 800 {
                save_jpeg(&current, &last_path)?;
            } else {
                let sized_path = thumb_dir.join(format!("{}_{}.jpg", image_id, size));
                save_jpeg(&current, &sized_path)?;
            }
            continue;
        }
        let resized = current.resize(size, size, FilterType::Lanczos3);
        if size == 800 {
            save_jpeg(&resized, &last_path)?;
        } else {
            let sized_path = thumb_dir.join(format!("{}_{}.jpg", image_id, size));
            save_jpeg(&resized, &sized_path)?;
        }
        current = resized;
    }
    Ok(last_path)
}

pub fn thumbnail_path(app_data_dir: &Path, image_id: &str) -> PathBuf {
    thumbnail_dir(app_data_dir).join(format!("{}.jpg", image_id))
}

pub fn sized_thumbnail_path(app_data_dir: &Path, image_id: &str, size: u32) -> PathBuf {
    if size == 800 {
        thumbnail_path(app_data_dir, image_id)
    } else {
        thumbnail_dir(app_data_dir).join(format!("{}_{}.jpg", image_id, size))
    }
}
