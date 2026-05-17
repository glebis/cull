pub mod fuji;
pub mod libraw;

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMetadata {
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub shutter_speed: Option<String>,
    pub aperture: Option<String>,
    pub iso: Option<u32>,
    pub focal_length: Option<f32>,
    pub date_taken: Option<String>,
    pub film_simulation: Option<String>,
    pub gps: Option<(f64, f64)>,
    pub sensor_width: u32,
    pub sensor_height: u32,
}

pub struct RawPreview {
    pub image: image::DynamicImage,
    pub metadata: RawMetadata,
}

pub trait RawDecoder: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String>;
}

pub fn decode_raw_preview(path: &Path) -> Result<RawPreview, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if ext.eq_ignore_ascii_case("raf") {
        match fuji::FujiRafDecoder.extract_preview(path) {
            Ok(preview) => return Ok(preview),
            Err(e) => {
                crate::safe_eprintln!("[raw] Fuji RAF parser failed, trying LibRaw: {}", e);
            }
        }
    }

    libraw::LibRawDecoder.extract_preview(path)
}
