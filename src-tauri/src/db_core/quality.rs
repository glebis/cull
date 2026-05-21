use crate::db_core::models::ImageQualityMetrics;
use image::GenericImageView;
use std::path::Path;

pub const QUALITY_ANALYZER_VERSION: &str = "quality-v1";

pub fn analyze_image_quality(
    image_id: &str,
    image_path: &Path,
) -> Result<ImageQualityMetrics, String> {
    let img = crate::db_core::image_decode::decode_image(image_path, false)?.image;
    let img = downsample_for_analysis(&img);
    let gray = img.to_luma8();
    let (width, height) = gray.dimensions();
    if width == 0 || height == 0 {
        return Err("Image has no pixels".to_string());
    }

    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut shadow = 0u64;
    let mut highlight = 0u64;
    let pixel_count = (width as u64).saturating_mul(height as u64).max(1);

    for pixel in gray.pixels() {
        let luma = pixel[0] as f64;
        sum += luma;
        sum_sq += luma * luma;
        if pixel[0] <= 3 {
            shadow += 1;
        }
        if pixel[0] >= 252 {
            highlight += 1;
        }
    }

    let mean = sum / pixel_count as f64;
    let variance = (sum_sq / pixel_count as f64) - (mean * mean);
    let contrast = variance.max(0.0).sqrt() / 255.0;
    let focus_score = laplacian_variance(&gray);
    let blur_score = 1.0 - (focus_score / (focus_score + 100.0));
    let clipped_shadow_pct = shadow as f64 / pixel_count as f64;
    let clipped_highlight_pct = highlight as f64 / pixel_count as f64;
    let mean_luma = mean / 255.0;
    let mean_penalty = (mean_luma - 0.5).abs() * 1.4;
    let clip_penalty = (clipped_shadow_pct + clipped_highlight_pct).min(1.0) * 4.0;
    let exposure_score = (1.0 - mean_penalty - clip_penalty * 0.6).clamp(0.0, 1.0);

    Ok(ImageQualityMetrics {
        image_id: image_id.to_string(),
        analyzer_version: QUALITY_ANALYZER_VERSION.to_string(),
        focus_score,
        blur_score: blur_score.clamp(0.0, 1.0),
        exposure_score,
        clipped_shadow_pct,
        clipped_highlight_pct,
        mean_luma,
        contrast,
        analyzed_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn downsample_for_analysis(img: &image::DynamicImage) -> image::DynamicImage {
    let (width, height) = img.dimensions();
    let max_side = width.max(height);
    if max_side <= 512 {
        return img.clone();
    }

    let scale = 512.0 / max_side as f32;
    let new_width = ((width as f32 * scale).round() as u32).max(1);
    let new_height = ((height as f32 * scale).round() as u32).max(1);
    img.resize(new_width, new_height, image::imageops::FilterType::Triangle)
}

fn laplacian_variance(gray: &image::GrayImage) -> f64 {
    let (width, height) = gray.dimensions();
    if width < 3 || height < 3 {
        return 0.0;
    }

    let mut values = Vec::with_capacity(((width - 2) * (height - 2)) as usize);
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let center = gray.get_pixel(x, y)[0] as f64;
            let up = gray.get_pixel(x, y - 1)[0] as f64;
            let down = gray.get_pixel(x, y + 1)[0] as f64;
            let left = gray.get_pixel(x - 1, y)[0] as f64;
            let right = gray.get_pixel(x + 1, y)[0] as f64;
            values.push((4.0 * center) - up - down - left - right);
        }
    }

    let count = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / count;
    values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / count
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn flat_image_has_low_focus_and_high_blur_score() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("flat.png");
        let img = ImageBuffer::from_pixel(32, 32, Rgb([128u8, 128, 128]));
        img.save(&path).unwrap();

        let metrics = analyze_image_quality("flat", &path).unwrap();
        assert!(metrics.focus_score < 1.0, "{:?}", metrics);
        assert!(metrics.blur_score > 0.95, "{:?}", metrics);
        assert!(metrics.exposure_score > 0.9, "{:?}", metrics);
    }

    #[test]
    fn high_contrast_edges_raise_focus_score() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("checker.png");
        let img = ImageBuffer::from_fn(32, 32, |x, y| {
            let v = if (x + y) % 2 == 0 { 0u8 } else { 255u8 };
            Rgb([v, v, v])
        });
        img.save(&path).unwrap();

        let metrics = analyze_image_quality("checker", &path).unwrap();
        assert!(metrics.focus_score > 1_000.0, "{:?}", metrics);
        assert!(metrics.blur_score < 0.1, "{:?}", metrics);
    }
}
