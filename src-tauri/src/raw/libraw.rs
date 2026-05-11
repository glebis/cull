use std::path::Path;
use super::{RawDecoder, RawPreview, RawMetadata};

pub struct LibRawDecoder;

impl RawDecoder for LibRawDecoder {
    fn extensions(&self) -> &[&str] {
        &["cr2", "cr3", "nef", "arw", "dng", "orf", "rw2"]
    }

    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("Failed to read RAW file: {}", e))?;

        let mut raw_image = rsraw::RawImage::open(&data)
            .map_err(|e| format!("LibRaw failed to open: {}", e))?;

        let thumbs = raw_image.extract_thumbs()
            .map_err(|e| format!("LibRaw failed to extract thumbnails: {}", e))?;

        let thumb = thumbs.into_iter()
            .max_by_key(|t| t.width as u64 * t.height as u64)
            .ok_or_else(|| "No thumbnails found in RAW file".to_string())?;

        let image = image::load_from_memory(&thumb.data)
            .map_err(|e| format!("Failed to decode extracted thumbnail: {}", e))?;

        let metadata = RawMetadata {
            camera_model: None,
            lens: None,
            shutter_speed: None,
            aperture: None,
            iso: None,
            focal_length: None,
            date_taken: None,
            film_simulation: None,
            gps: None,
            sensor_width: 0,
            sensor_height: 0,
        };

        Ok(RawPreview { image, metadata })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_libraw_decoder_extensions() {
        let decoder = LibRawDecoder;
        let exts = decoder.extensions();
        assert!(exts.contains(&"cr2"));
        assert!(exts.contains(&"nef"));
        assert!(exts.contains(&"arw"));
        assert!(exts.contains(&"dng"));
        assert!(!exts.contains(&"raf"));
    }
}
