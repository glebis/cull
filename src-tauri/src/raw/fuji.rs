use std::path::Path;
use super::{RawDecoder, RawPreview, RawMetadata};

pub struct FujiRafDecoder;

const RAF_MAGIC: &[u8; 16] = b"FUJIFILMCCD-RAW ";
const JPEG_OFFSET_POS: usize = 84;
const JPEG_LENGTH_POS: usize = 88;
const MIN_HEADER_SIZE: usize = 100;

#[derive(Debug)]
struct RafHeader {
    jpeg_offset: u32,
    jpeg_length: u32,
    camera_model: String,
}

fn parse_raf_header(data: &[u8]) -> Result<RafHeader, String> {
    if data.len() < MIN_HEADER_SIZE {
        return Err(format!("File too small for RAF: {} bytes", data.len()));
    }

    if &data[0..16] != RAF_MAGIC {
        return Err("Invalid RAF magic bytes".to_string());
    }

    let jpeg_offset = u32::from_be_bytes(
        data[JPEG_OFFSET_POS..JPEG_OFFSET_POS + 4]
            .try_into()
            .map_err(|_| "Failed to read JPEG offset")?
    );
    let jpeg_length = u32::from_be_bytes(
        data[JPEG_LENGTH_POS..JPEG_LENGTH_POS + 4]
            .try_into()
            .map_err(|_| "Failed to read JPEG length")?
    );

    let model_bytes = &data[28..60];
    let camera_model = std::str::from_utf8(model_bytes)
        .unwrap_or("")
        .trim_end_matches('\0')
        .trim()
        .to_string();

    if jpeg_offset == 0 {
        return Err("RAF JPEG offset is zero".to_string());
    }
    if jpeg_length == 0 {
        return Err("RAF JPEG length is zero".to_string());
    }

    Ok(RafHeader {
        jpeg_offset,
        jpeg_length,
        camera_model,
    })
}

fn extract_embedded_jpeg<'a>(data: &'a [u8], header: &RafHeader) -> Result<&'a [u8], String> {
    let start = header.jpeg_offset as usize;
    let end = (start as u64)
        .checked_add(header.jpeg_length as u64)
        .ok_or_else(|| "JPEG offset + length overflow".to_string())? as usize;

    if end > data.len() {
        return Err(format!(
            "JPEG range {}..{} exceeds file size {}",
            start, end, data.len()
        ));
    }

    let jpeg = &data[start..end];

    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return Err("Embedded data does not start with JPEG SOI marker".to_string());
    }

    if jpeg.len() < 2 || jpeg[jpeg.len() - 2] != 0xFF || jpeg[jpeg.len() - 1] != 0xD9 {
        return Err("Embedded JPEG missing EOI marker".to_string());
    }

    Ok(jpeg)
}

impl RawDecoder for FujiRafDecoder {
    fn extensions(&self) -> &[&str] {
        &["raf"]
    }

    fn extract_preview(&self, path: &Path) -> Result<RawPreview, String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("Failed to read RAF file: {}", e))?;

        let header = parse_raf_header(&data)?;
        let jpeg_data = extract_embedded_jpeg(&data, &header)?;

        let image = image::load_from_memory_with_format(jpeg_data, image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to decode embedded JPEG: {}", e))?;

        let metadata = RawMetadata {
            camera_model: if header.camera_model.is_empty() { None } else { Some(header.camera_model) },
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
    fn test_raf_magic_validation() {
        let bad_data = b"NOT A RAF FILE AT ALL!!";
        assert!(parse_raf_header(bad_data).is_err());
    }

    #[test]
    fn test_raf_magic_too_short() {
        let short = b"FUJI";
        assert!(parse_raf_header(short).is_err());
    }

    #[test]
    fn test_raf_magic_accepted() {
        let mut data = b"FUJIFILMCCD-RAW 0201".to_vec();
        data.resize(120, 0);
        let result = parse_raf_header(&data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.contains("magic"), "Should pass magic check: {}", err);
    }

    #[test]
    fn test_extract_jpeg_validates_soi_eoi() {
        let jpeg_blob = build_minimal_jpeg();
        let raf_data = build_test_raf(&jpeg_blob);
        let header = parse_raf_header(&raf_data).unwrap();
        let jpeg = extract_embedded_jpeg(&raf_data, &header).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    }

    fn build_minimal_jpeg() -> Vec<u8> {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00]);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    fn build_test_raf(jpeg_blob: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"FUJIFILMCCD-RAW ");
        buf.extend_from_slice(b"0201");
        buf.extend_from_slice(b"GFX100S\0");
        let mut model = b"GFX 100S".to_vec();
        model.resize(32, 0);
        buf.extend_from_slice(&model);
        buf.extend_from_slice(b"0100");
        buf.resize(84, 0);
        let jpeg_offset: u32 = 100;
        buf.extend_from_slice(&jpeg_offset.to_be_bytes());
        let jpeg_len = jpeg_blob.len() as u32;
        buf.extend_from_slice(&jpeg_len.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.resize(jpeg_offset as usize, 0);
        buf.extend_from_slice(jpeg_blob);
        buf
    }

    #[test]
    #[ignore] // requires real RAF file on disk
    fn test_real_gfx_raf() {
        let path = std::path::Path::new(
            "/Users/glebkalinin/Documents/Pictures/20210317 a53/Capture/20210317 a53 0028.RAF"
        );
        if !path.exists() {
            eprintln!("Skipping: test RAF file not found");
            return;
        }
        let preview = FujiRafDecoder.extract_preview(path).unwrap();
        assert!(preview.image.width() > 0, "Preview width should be > 0");
        assert!(preview.image.height() > 0, "Preview height should be > 0");
        assert!(preview.image.width() >= 1000, "Preview should be at least 1000px wide, got {}", preview.image.width());
        eprintln!("Preview: {}x{}", preview.image.width(), preview.image.height());
        eprintln!("Camera: {:?}", preview.metadata.camera_model);
        assert_eq!(preview.metadata.camera_model.as_deref(), Some("GFX 50R"));
    }

    #[test]
    #[ignore]
    fn test_real_gfx_raf_via_libraw() {
        let path = std::path::Path::new(
            "/Users/glebkalinin/Documents/Pictures/20210317 a53/Capture/20210317 a53 0028.RAF"
        );
        if !path.exists() { return; }
        let preview = super::super::decode_raw_preview(path).unwrap();
        assert!(preview.image.width() > 0);
        eprintln!("decode_raw_preview: {}x{}", preview.image.width(), preview.image.height());
    }
}
