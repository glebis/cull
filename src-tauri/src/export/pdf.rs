use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

pub fn assemble_pdf(
    image_paths: &[String],
    width_px: u32,
    height_px: u32,
    output_path: &str,
) -> Result<String, String> {
    let dpi = 150.0;
    let width_mm = Mm((width_px as f32 / dpi) * 25.4);
    let height_mm = Mm((height_px as f32 / dpi) * 25.4);

    let (doc, mut page_idx, mut layer_idx) =
        PdfDocument::new("Cull Export", width_mm, height_mm, "Layer 1");

    for (i, img_path) in image_paths.iter().enumerate() {
        if i > 0 {
            let (new_page, new_layer) =
                doc.add_page(width_mm, height_mm, format!("Layer {}", i + 1));
            page_idx = new_page;
            layer_idx = new_layer;
        }

        let current_layer = doc.get_page(page_idx).get_layer(layer_idx);

        let png_bytes =
            std::fs::read(img_path).map_err(|e| format!("Failed to read '{}': {}", img_path, e))?;

        let decoder = png::Decoder::new(std::io::Cursor::new(&png_bytes));
        let mut reader = decoder
            .read_info()
            .map_err(|e| format!("Failed to decode PNG '{}': {}", img_path, e))?;

        let output_buffer_size = reader
            .output_buffer_size()
            .ok_or_else(|| format!("Failed to determine PNG buffer size for '{}'", img_path))?;
        let mut buf = vec![0; output_buffer_size];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| format!("Failed to read PNG frame: {}", e))?;
        buf.truncate(info.buffer_size());

        let img_w = info.width as usize;
        let img_h = info.height as usize;

        let (color_space, pixel_data) = match info.color_type {
            png::ColorType::Rgba => {
                // Strip alpha channel for PDF (RGB only)
                let mut rgb = Vec::with_capacity(img_w * img_h * 3);
                for chunk in buf.chunks(4) {
                    rgb.push(chunk[0]);
                    rgb.push(chunk[1]);
                    rgb.push(chunk[2]);
                }
                (ColorSpace::Rgb, rgb)
            }
            png::ColorType::Rgb => (ColorSpace::Rgb, buf),
            png::ColorType::Grayscale => (ColorSpace::Greyscale, buf),
            _ => return Err(format!("Unsupported color type in '{}'", img_path)),
        };

        let image_data = ImageXObject {
            width: Px(img_w),
            height: Px(img_h),
            color_space,
            bits_per_component: ColorBits::Bit8,
            interpolate: true,
            image_data: pixel_data,
            image_filter: None,
            clipping_bbox: None,
            smask: None,
        };

        let pdf_image = Image::from(image_data);
        pdf_image.add_to_layer(
            current_layer,
            ImageTransform {
                translate_x: Some(Mm(0.0)),
                translate_y: Some(Mm(0.0)),
                scale_x: Some(width_mm.0 / (img_w as f32 / dpi * 25.4)),
                scale_y: Some(height_mm.0 / (img_h as f32 / dpi * 25.4)),
                ..Default::default()
            },
        );
    }

    let output =
        File::create(output_path).map_err(|e| format!("Failed to create output file: {}", e))?;
    doc.save(&mut BufWriter::new(output))
        .map_err(|e| format!("Failed to save PDF: {}", e))?;

    Ok(output_path.to_string())
}
