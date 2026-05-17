use crate::db_core::models::{ImageColorMetrics, ImagePaletteColor};
use image::GenericImageView;
use std::collections::HashMap;
use std::path::Path;

pub const COLOR_ANALYZER_VERSION: &str = "color-v1";
const PALETTE_SIZE: usize = 6;

#[derive(Debug, Clone, Default)]
struct ColorBin {
    count: u64,
    red_sum: u64,
    green_sum: u64,
    blue_sum: u64,
}

pub fn analyze_image_color_metrics(
    image_id: &str,
    image_path: &Path,
) -> Result<ImageColorMetrics, String> {
    let img = image::open(image_path).map_err(|e| format!("Image open error: {}", e))?;
    Ok(analyze_dynamic_image_color_metrics(image_id, &img))
}

pub fn analyze_dynamic_image_color_metrics(
    image_id: &str,
    img: &image::DynamicImage,
) -> ImageColorMetrics {
    let img = downsample_for_color_analysis(img).to_rgba8();
    let (width, height) = img.dimensions();
    let capacity = (width as u64).saturating_mul(height as u64).max(1);
    let mut bins: HashMap<u16, ColorBin> = HashMap::new();
    let mut pixel_count = 0u64;
    let mut red_sum = 0.0f64;
    let mut green_sum = 0.0f64;
    let mut blue_sum = 0.0f64;
    let mut luma_sum = 0.0f64;
    let mut luma_sum_sq = 0.0f64;
    let mut saturation_sum = 0.0f64;
    let mut rg_values = Vec::with_capacity(capacity.min(16_384) as usize);
    let mut yb_values = Vec::with_capacity(capacity.min(16_384) as usize);
    let mut sampled_pixels = Vec::with_capacity(capacity.min(16_384) as usize);

    for pixel in img.pixels() {
        let alpha = pixel[3];
        if alpha < 16 {
            continue;
        }

        let red = pixel[0];
        let green = pixel[1];
        let blue = pixel[2];
        let (_hue, saturation, _value) = rgb_to_hsv(red, green, blue);
        let luma = relative_luma(red, green, blue);

        let key = quantized_key(red, green, blue);
        let bin = bins.entry(key).or_default();
        bin.count += 1;
        bin.red_sum += red as u64;
        bin.green_sum += green as u64;
        bin.blue_sum += blue as u64;

        pixel_count += 1;
        red_sum += red as f64;
        green_sum += green as f64;
        blue_sum += blue as f64;
        luma_sum += luma;
        luma_sum_sq += luma * luma;
        saturation_sum += saturation;
        rg_values.push(red as f64 - green as f64);
        yb_values.push(((red as f64 + green as f64) * 0.5) - blue as f64);
        sampled_pixels.push([red, green, blue]);
    }

    if pixel_count == 0 {
        return empty_metrics(image_id);
    }

    let palette = build_palette(&sampled_pixels, bins);
    let dominant = palette
        .first()
        .cloned()
        .unwrap_or_else(|| average_palette_color(red_sum, green_sum, blue_sum, pixel_count));
    let (_hue, dominant_saturation, dominant_value) =
        rgb_to_hsv(dominant.red, dominant.green, dominant.blue);
    let dominant_hue_bucket = hue_bucket(dominant.red, dominant.green, dominant.blue);
    let mean_luma = luma_sum / pixel_count as f64;
    let contrast = ((luma_sum_sq / pixel_count as f64) - (mean_luma * mean_luma))
        .max(0.0)
        .sqrt();
    let mean_saturation = saturation_sum / pixel_count as f64;
    let colorfulness = colorfulness(&rg_values, &yb_values);

    ImageColorMetrics {
        image_id: image_id.to_string(),
        analyzer_version: COLOR_ANALYZER_VERSION.to_string(),
        dominant_hex: dominant.hex,
        palette,
        dominant_hue_bucket: if dominant_value < 0.08 || dominant_saturation < 0.12 {
            "mono".to_string()
        } else {
            dominant_hue_bucket
        },
        mean_luma: mean_luma.clamp(0.0, 1.0),
        mean_saturation: mean_saturation.clamp(0.0, 1.0),
        colorfulness: colorfulness.clamp(0.0, 1.0),
        contrast: contrast.clamp(0.0, 1.0),
        analyzed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn downsample_for_color_analysis(img: &image::DynamicImage) -> image::DynamicImage {
    let (width, height) = img.dimensions();
    let max_side = width.max(height);
    if max_side <= 160 {
        return img.clone();
    }

    let scale = 160.0 / max_side as f32;
    let new_width = ((width as f32 * scale).round() as u32).max(1);
    let new_height = ((height as f32 * scale).round() as u32).max(1);
    img.resize(new_width, new_height, image::imageops::FilterType::Triangle)
}

fn build_palette(pixels: &[[u8; 3]], bins: HashMap<u16, ColorBin>) -> Vec<ImagePaletteColor> {
    if pixels.is_empty() {
        return vec![];
    }

    let mut centroids = initial_centroids(bins);
    if centroids.is_empty() {
        return vec![];
    }

    for _ in 0..8 {
        let mut sums = vec![[0u64; 3]; centroids.len()];
        let mut counts = vec![0u64; centroids.len()];

        for pixel in pixels {
            let idx = nearest_centroid(pixel, &centroids);
            sums[idx][0] += pixel[0] as u64;
            sums[idx][1] += pixel[1] as u64;
            sums[idx][2] += pixel[2] as u64;
            counts[idx] += 1;
        }

        for (idx, centroid) in centroids.iter_mut().enumerate() {
            if counts[idx] == 0 {
                continue;
            }
            centroid[0] = sums[idx][0] as f64 / counts[idx] as f64;
            centroid[1] = sums[idx][1] as f64 / counts[idx] as f64;
            centroid[2] = sums[idx][2] as f64 / counts[idx] as f64;
        }
    }

    let mut counts = vec![0u64; centroids.len()];
    for pixel in pixels {
        let idx = nearest_centroid(pixel, &centroids);
        counts[idx] += 1;
    }

    let mut palette: Vec<(ImagePaletteColor, u64)> = centroids
        .into_iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(centroid, count)| {
            let red = centroid[0].round().clamp(0.0, 255.0) as u8;
            let green = centroid[1].round().clamp(0.0, 255.0) as u8;
            let blue = centroid[2].round().clamp(0.0, 255.0) as u8;
            (
                ImagePaletteColor {
                    hex: rgb_hex(red, green, blue),
                    red,
                    green,
                    blue,
                    percentage: (count as f64 / pixels.len() as f64).clamp(0.0, 1.0),
                },
                count,
            )
        })
        .collect();
    palette.sort_by(|a, b| b.1.cmp(&a.1));
    palette.into_iter().map(|(color, _)| color).collect()
}

fn initial_centroids(bins: HashMap<u16, ColorBin>) -> Vec<[f64; 3]> {
    let mut bins: Vec<ColorBin> = bins.into_values().collect();
    bins.sort_by(|a, b| b.count.cmp(&a.count));
    bins.into_iter()
        .take(PALETTE_SIZE)
        .filter(|bin| bin.count > 0)
        .map(|bin| {
            [
                bin.red_sum as f64 / bin.count as f64,
                bin.green_sum as f64 / bin.count as f64,
                bin.blue_sum as f64 / bin.count as f64,
            ]
        })
        .collect()
}

fn nearest_centroid(pixel: &[u8; 3], centroids: &[[f64; 3]]) -> usize {
    centroids
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = color_distance_sq(pixel, a);
            let db = color_distance_sq(pixel, b);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn color_distance_sq(pixel: &[u8; 3], centroid: &[f64; 3]) -> f64 {
    let dr = pixel[0] as f64 - centroid[0];
    let dg = pixel[1] as f64 - centroid[1];
    let db = pixel[2] as f64 - centroid[2];
    (dr * dr) + (dg * dg) + (db * db)
}

fn empty_metrics(image_id: &str) -> ImageColorMetrics {
    ImageColorMetrics {
        image_id: image_id.to_string(),
        analyzer_version: COLOR_ANALYZER_VERSION.to_string(),
        dominant_hex: "#000000".to_string(),
        palette: vec![],
        dominant_hue_bucket: "mono".to_string(),
        mean_luma: 0.0,
        mean_saturation: 0.0,
        colorfulness: 0.0,
        contrast: 0.0,
        analyzed_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn average_palette_color(
    red_sum: f64,
    green_sum: f64,
    blue_sum: f64,
    pixel_count: u64,
) -> ImagePaletteColor {
    let count = pixel_count.max(1) as f64;
    let red = (red_sum / count).round().clamp(0.0, 255.0) as u8;
    let green = (green_sum / count).round().clamp(0.0, 255.0) as u8;
    let blue = (blue_sum / count).round().clamp(0.0, 255.0) as u8;
    ImagePaletteColor {
        hex: rgb_hex(red, green, blue),
        red,
        green,
        blue,
        percentage: 1.0,
    }
}

fn quantized_key(red: u8, green: u8, blue: u8) -> u16 {
    let r = (red >> 3) as u16;
    let g = (green >> 3) as u16;
    let b = (blue >> 3) as u16;
    (r << 10) | (g << 5) | b
}

fn relative_luma(red: u8, green: u8, blue: u8) -> f64 {
    ((0.2126 * red as f64) + (0.7152 * green as f64) + (0.0722 * blue as f64)) / 255.0
}

fn rgb_to_hsv(red: u8, green: u8, blue: u8) -> (f64, f64, f64) {
    let r = red as f64 / 255.0;
    let g = green as f64 / 255.0;
    let b = blue as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    (hue, saturation, max)
}

fn hue_bucket(red: u8, green: u8, blue: u8) -> String {
    let (hue, saturation, value) = rgb_to_hsv(red, green, blue);
    if value < 0.08 || saturation < 0.12 {
        return "mono".to_string();
    }

    match hue {
        h if !(15.0..345.0).contains(&h) => "red",
        h if h < 45.0 => "orange",
        h if h < 70.0 => "yellow",
        h if h < 165.0 => "green",
        h if h < 195.0 => "cyan",
        h if h < 255.0 => "blue",
        h if h < 285.0 => "purple",
        _ => "magenta",
    }
    .to_string()
}

fn colorfulness(rg_values: &[f64], yb_values: &[f64]) -> f64 {
    let rg_std = stddev(rg_values);
    let yb_std = stddev(yb_values);
    let rg_mean = mean(rg_values).abs();
    let yb_mean = mean(yb_values).abs();
    let raw =
        (rg_std.powi(2) + yb_std.powi(2)).sqrt() + 0.3 * (rg_mean.powi(2) + yb_mean.powi(2)).sqrt();
    (raw / 255.0).clamp(0.0, 1.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let avg = mean(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = value - avg;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn rgb_hex(red: u8, green: u8, blue: u8) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}
