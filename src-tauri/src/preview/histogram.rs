use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistogramSource {
    Original,
    Thumbnail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageHistogram {
    pub image_id: String,
    pub source: HistogramSource,
    pub pixel_count: u64,
    pub red: Vec<u32>,
    pub green: Vec<u32>,
    pub blue: Vec<u32>,
    pub luma: Vec<u32>,
}

pub fn compute_image_histogram(
    image_id: &str,
    image: &image::DynamicImage,
    source: HistogramSource,
) -> ImageHistogram {
    let mut red = vec![0u32; 256];
    let mut green = vec![0u32; 256];
    let mut blue = vec![0u32; 256];
    let mut luma = vec![0u32; 256];
    let mut pixel_count = 0u64;
    let rgba = image.to_rgba8();

    for pixel in rgba.pixels() {
        let [r, g, b, alpha] = pixel.0;
        if alpha == 0 {
            continue;
        }
        pixel_count += 1;
        red[r as usize] += 1;
        green[g as usize] += 1;
        blue[b as usize] += 1;
        let y = ((u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000) as usize;
        luma[y] += 1;
    }

    ImageHistogram {
        image_id: image_id.to_string(),
        source,
        pixel_count,
        red,
        green,
        blue,
        luma,
    }
}

pub fn load_image_histogram(
    image: &ImageWithFile,
    app_data_dir: &Path,
) -> Result<ImageHistogram, String> {
    let (path, source) = histogram_source_path(image, app_data_dir);
    load_image_histogram_from_source(&image.image.id, &path, source)
}

pub fn load_image_histogram_from_source(
    image_id: &str,
    path: &Path,
    source: HistogramSource,
) -> Result<ImageHistogram, String> {
    let decoded = crate::db_core::image_decode::decode_image(path, true).map_err(|e| {
        format!(
            "Failed to decode histogram source '{}': {}",
            path.display(),
            e
        )
    })?;
    Ok(compute_image_histogram(image_id, &decoded.image, source))
}

pub fn histogram_source_path(
    image: &ImageWithFile,
    app_data_dir: &Path,
) -> (PathBuf, HistogramSource) {
    let source = PathBuf::from(&image.path);
    let ext = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if source.exists() && !crate::extensions::should_use_thumbnail_for_ml(ext) {
        return (source, HistogramSource::Original);
    }

    (
        thumbnails::thumbnail_path(app_data_dir, &image.image.id),
        HistogramSource::Thumbnail,
    )
}
