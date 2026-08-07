use std::path::Path;

#[cfg(target_os = "macos")]
const PLATFORM_DECODE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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

    decode_with_sips(path, None)
}

#[cfg(target_os = "macos")]
pub(crate) fn decode_pdf_preview_with_platform(
    path: &Path,
    max_dimension: u32,
) -> Result<image::DynamicImage, String> {
    decode_with_sips(path, Some(max_dimension))
}

#[cfg(target_os = "macos")]
fn decode_with_sips(
    path: &Path,
    max_dimension: Option<u32>,
) -> Result<image::DynamicImage, String> {
    use std::process::Stdio;

    let temp = tempfile::Builder::new()
        .prefix("cull-decode-")
        .suffix(".png")
        .tempfile()
        .map_err(|e| format!("Failed to create decode temp file: {}", e))?;

    let mut command = std::process::Command::new("/usr/bin/sips");
    command.arg("-s").arg("format").arg("png");
    if let Some(max_dimension) = max_dimension {
        command
            .arg("--resampleHeightWidthMax")
            .arg(max_dimension.to_string());
    }
    let mut child = command
        .arg(path)
        .arg("--out")
        .arg(temp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to run macOS ImageIO decoder: {}", e))?;

    let status = wait_for_child_with_timeout(&mut child, PLATFORM_DECODE_TIMEOUT)?;
    if !status.success() {
        return Err(format!("sips exited with {}", status));
    }

    image::open(temp.path()).map_err(|e| format!("sips produced unreadable PNG: {}", e))
}

#[cfg(target_os = "macos")]
fn wait_for_child_with_timeout(
    child: &mut std::process::Child,
    timeout: std::time::Duration,
) -> Result<std::process::ExitStatus, String> {
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "macOS ImageIO decoder timed out after {} seconds",
                    timeout.as_secs_f32()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "Failed waiting for macOS ImageIO decoder: {}",
                    error
                ))
            }
        }
    }
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

    #[cfg(target_os = "macos")]
    #[test]
    fn decodes_a_real_pdf_first_page_with_the_bounded_macos_fallback() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf/sample_two_page.pdf");

        let preview = decode_pdf_preview_with_platform(&path, 1200).unwrap();

        assert!(preview.width() > 0);
        assert!(preview.height() > preview.width());
        assert!(preview.width().max(preview.height()) <= 1200);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn platform_decoder_terminates_a_child_that_exceeds_its_deadline() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let started = std::time::Instant::now();

        let result = wait_for_child_with_timeout(&mut child, std::time::Duration::from_millis(25));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert!(child.try_wait().unwrap().is_some());
    }
}
