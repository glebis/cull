use std::path::Path;

#[derive(Debug)]
pub struct DecodedImage {
    pub image: image::DynamicImage,
    pub raw_metadata: Option<crate::raw::RawMetadata>,
}

pub fn decode_image(path: &Path, module_raw: bool) -> Result<DecodedImage, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if crate::extensions::is_raw_extension(&ext) {
        if !module_raw {
            return Err(format!("RAW support is disabled for .{} files", ext));
        }
        let preview = crate::raw::decode_raw_preview(path)
            .map_err(|e| format!("RAW decode failed: {}", e))?;
        return Ok(DecodedImage {
            image: preview.image,
            raw_metadata: Some(preview.metadata),
        });
    }

    match image::open(path) {
        Ok(image) => Ok(DecodedImage {
            image,
            raw_metadata: None,
        }),
        Err(image_error) => decode_with_platform(path, &ext)
            .map(|image| DecodedImage {
                image,
                raw_metadata: None,
            })
            .map_err(|platform_error| {
                if crate::extensions::is_platform_decodable(&ext) {
                    format!(
                        "Image crate decode failed: {}; platform decode failed: {}",
                        image_error, platform_error
                    )
                } else {
                    format!("Image open error: {}", image_error)
                }
            }),
    }
}

#[cfg(target_os = "macos")]
fn decode_with_platform(path: &Path, ext: &str) -> Result<image::DynamicImage, String> {
    if !crate::extensions::is_platform_decodable(ext) {
        return Err(format!("No platform decoder configured for .{} files", ext));
    }

    let temp = tempfile::Builder::new()
        .prefix("cull-decode-")
        .suffix(".png")
        .tempfile()
        .map_err(|e| format!("Failed to create decode temp file: {}", e))?;

    let output = std::process::Command::new("/usr/bin/sips")
        .arg("-s")
        .arg("format")
        .arg("png")
        .arg(path)
        .arg("--out")
        .arg(temp.path())
        .output()
        .map_err(|e| format!("Failed to run macOS ImageIO decoder: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(format!("sips exited with {}: {}", output.status, detail));
    }

    image::open(temp.path()).map_err(|e| format!("sips produced unreadable PNG: {}", e))
}

#[cfg(not(target_os = "macos"))]
fn decode_with_platform(_path: &Path, ext: &str) -> Result<image::DynamicImage, String> {
    Err(format!("No platform decoder configured for .{} files", ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_decode_failure_returns_error_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("broken.cr2");
        std::fs::write(&path, b"not a real raw file").unwrap();

        // With module_raw on by default, corrupt RAW input must surface as Err.
        let result = decode_image(&path, true);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("RAW decode failed"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decodes_svg_with_macos_imageio_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vector.svg");
        std::fs::write(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48"><rect width="64" height="48" fill="#7aa2f7"/></svg>"##,
        )
        .unwrap();

        let decoded = decode_image(&path, false).unwrap();

        assert_eq!(decoded.image.width(), 64);
        assert_eq!(decoded.image.height(), 48);
        assert!(decoded.raw_metadata.is_none());
    }
}
