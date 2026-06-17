use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, Rgb, RgbImage};
use pdfium_render::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const JPEG_QUALITY: u8 = 90;
pub const THUMBNAIL_SIZES: [u32; 4] = [64, 128, 256, 800];
const DOCUMENT_PREVIEW_DIMENSION: u32 = 1200;

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
    let img = crate::db_core::image_decode::decode_image(source_path, false)?.image;
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

pub fn generate_document_thumbnail(
    source_path: &Path,
    app_data_dir: &Path,
    image_id: &str,
) -> Result<PathBuf, String> {
    let thumb_path = thumbnail_path(app_data_dir, image_id);
    if thumb_path.exists() {
        return Ok(thumb_path);
    }

    let preview = render_pdf_first_page_thumbnail(source_path)
        .unwrap_or_else(|_| placeholder_document_thumbnail());
    generate_thumbnail_from_image(&preview, app_data_dir, image_id)
}

pub fn read_pdf_page_count(source_path: &Path) -> Result<u32, String> {
    with_open_pdf(source_path, |document| Ok(document.pages().len() as u32))
}

pub fn read_pdf_title(source_path: &Path) -> Result<Option<String>, String> {
    with_open_pdf(source_path, |document| {
        Ok(document
            .metadata()
            .get(PdfDocumentMetadataTagType::Title)
            .and_then(|tag| {
                let value = tag.value().trim();
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            }))
    })
}

pub fn read_pdf_page_texts(source_path: &Path) -> Result<Vec<(u32, Option<String>)>, String> {
    with_open_pdf(source_path, |document| {
        let mut result = Vec::with_capacity(document.pages().len() as usize);
        for (index, page) in document.pages().iter().enumerate() {
            let extracted = page
                .text()
                .map(|text| {
                    let text = text.all();
                    let text = text.trim();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text.to_string())
                    }
                })
                .ok()
                .flatten();
            result.push((index as u32, extracted));
        }

        Ok(result)
    })
}

pub fn read_pdf_page_metrics(
    source_path: &Path,
) -> Result<Vec<(u32, Option<f64>, Option<f64>)>, String> {
    with_open_pdf(source_path, |document| {
        let mut metrics = Vec::with_capacity(document.pages().len() as usize);
        for (index, page) in document.pages().iter().enumerate() {
            // Read intrinsic page dimensions in PDF points (1/72 inch) directly;
            // rendering a raster just to learn the size both wastes work and
            // records the wrong unit.
            metrics.push((
                index as u32,
                Some(page.width().value as f64),
                Some(page.height().value as f64),
            ));
        }

        Ok(metrics)
    })
}

fn render_pdf_first_page_thumbnail(source_path: &Path) -> Result<DynamicImage, String> {
    with_open_pdf(source_path, |document| {
        let page = document
            .pages()
            .first()
            .map_err(|e| format!("No pages in PDF: {}", e))?;

        let render_config = PdfRenderConfig::new()
            .set_maximum_width(DOCUMENT_PREVIEW_DIMENSION as i32)
            .set_maximum_height(DOCUMENT_PREVIEW_DIMENSION as i32);

        let rendered = page
            .render_with_config(&render_config)
            .map_err(|e| format!("PDF render failed: {}", e))?;
        let image = rendered.as_image();
        Ok(image)
    })
}

/// Bind the PDFium native library. Resolution order:
/// 1. `CULL_PDFIUM_PATH` — an explicit `libpdfium` file, or a directory containing it.
/// 2. The current working / executable directory (`./`).
/// 3. The system library search path.
///
/// PDFium is a runtime dependency that is not bundled with the crate; this lets
/// deployments and CI point at a provisioned binary without code changes.
fn bind_pdfium() -> Result<Pdfium, String> {
    if let Some(path) = std::env::var_os("CULL_PDFIUM_PATH") {
        let path = PathBuf::from(path);
        let lib = if path.is_dir() {
            Pdfium::pdfium_platform_library_name_at_path(&path)
        } else {
            path
        };
        return Pdfium::bind_to_library(lib)
            .map(Pdfium::new)
            .map_err(|e| format!("Failed to bind PDFium from CULL_PDFIUM_PATH: {}", e));
    }

    Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
        .or_else(|_| Pdfium::bind_to_system_library())
        .map(Pdfium::new)
        .map_err(|e| format!("Failed to bind PDFium: {}", e))
}

fn with_open_pdf<T, F>(source_path: &Path, operation: F) -> Result<T, String>
where
    F: for<'a> FnOnce(PdfDocument<'a>) -> Result<T, String>,
{
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(source_path, None)
        .map_err(|e| format!("Failed to open PDF: {}", e))?;

    operation(document)
}

fn placeholder_document_thumbnail() -> DynamicImage {
    let placeholder = RgbImage::from_fn(
        DOCUMENT_PREVIEW_DIMENSION,
        DOCUMENT_PREVIEW_DIMENSION,
        |_x, _y| {
            if (_x + _y) % 80 < 2 {
                Rgb([22, 30, 44])
            } else {
                Rgb([16, 20, 30])
            }
        },
    );
    DynamicImage::ImageRgb8(placeholder)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// A deterministic two-page US-Letter PDF (Helvetica text + `/Title`),
    /// generated by `tests/fixtures/pdf/sample_two_page.pdf`.
    fn fixture_pdf() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf/sample_two_page.pdf")
    }

    /// PDFium is an unbundled runtime dependency. When it cannot be bound (no
    /// provisioned binary locally or in CI), skip the extraction tests rather
    /// than report a false failure. Set `CULL_PDFIUM_PATH` to exercise them.
    fn require_pdfium() -> bool {
        if bind_pdfium().is_ok() {
            return true;
        }
        eprintln!(
            "skipping PDFium fixture test: no PDFium library bound \
             (set CULL_PDFIUM_PATH to a libpdfium file or directory)"
        );
        false
    }

    #[test]
    fn read_pdf_page_count_rejects_invalid_pdf_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, b"not a pdf").unwrap();
        assert!(read_pdf_page_count(path).is_err());
    }

    #[test]
    fn fixture_page_count_is_two() {
        if !require_pdfium() {
            return;
        }
        assert_eq!(read_pdf_page_count(&fixture_pdf()).unwrap(), 2);
    }

    #[test]
    fn fixture_page_metrics_are_us_letter_points() {
        if !require_pdfium() {
            return;
        }
        let metrics = read_pdf_page_metrics(&fixture_pdf()).unwrap();
        assert_eq!(metrics.len(), 2);
        for (index, (page_index, width, height)) in metrics.iter().enumerate() {
            assert_eq!(*page_index, index as u32);
            // US Letter = 612 x 792 points. Stored in points, not rendered pixels.
            assert!((width.unwrap() - 612.0).abs() < 1.0, "width {:?}", width);
            assert!((height.unwrap() - 792.0).abs() < 1.0, "height {:?}", height);
        }
    }

    #[test]
    fn fixture_title_is_extracted() {
        if !require_pdfium() {
            return;
        }
        assert_eq!(
            read_pdf_title(&fixture_pdf()).unwrap().as_deref(),
            Some("Cull Test Fixture")
        );
    }

    #[test]
    fn fixture_text_is_extracted_per_page() {
        if !require_pdfium() {
            return;
        }
        let texts = read_pdf_page_texts(&fixture_pdf()).unwrap();
        assert_eq!(texts.len(), 2);
        let page_one = texts[0].1.as_deref().unwrap_or("");
        let page_two = texts[1].1.as_deref().unwrap_or("");
        assert!(page_one.contains("page one"), "page 0 text: {:?}", page_one);
        assert!(page_two.contains("page two"), "page 1 text: {:?}", page_two);
    }
}
