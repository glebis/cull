use crate::db_core::models::ImagePerceptualHash;
use image::imageops::FilterType;
use std::f64::consts::PI;
use std::path::Path;

pub const PHASH_ALGORITHM: &str = "phash-dct-64-v1";
const PHASH_SIZE: usize = 32;
const DCT_SIZE: usize = 8;

pub fn analyze_image_perceptual_hash(
    image_id: &str,
    image_path: &Path,
) -> Result<ImagePerceptualHash, String> {
    let img = crate::db_core::image_decode::decode_image(image_path, false)?.image;
    Ok(analyze_dynamic_image_perceptual_hash(image_id, &img))
}

pub fn analyze_dynamic_image_perceptual_hash(
    image_id: &str,
    img: &image::DynamicImage,
) -> ImagePerceptualHash {
    let resized = img
        .resize_exact(PHASH_SIZE as u32, PHASH_SIZE as u32, FilterType::Triangle)
        .to_luma8();
    let mut pixels = [[0.0f64; PHASH_SIZE]; PHASH_SIZE];
    for (y, row) in pixels.iter_mut().enumerate().take(PHASH_SIZE) {
        for (x, pixel) in row.iter_mut().enumerate().take(PHASH_SIZE) {
            *pixel = resized.get_pixel(x as u32, y as u32)[0] as f64;
        }
    }

    let mut coeffs = [0.0f64; DCT_SIZE * DCT_SIZE];
    for v in 0..DCT_SIZE {
        for u in 0..DCT_SIZE {
            coeffs[v * DCT_SIZE + u] = dct_coefficient(&pixels, u, v);
        }
    }

    let median = median_without_dc(&coeffs);
    let mut hash = 0u64;
    for (idx, coeff) in coeffs.iter().enumerate() {
        if *coeff > median {
            hash |= 1u64 << (63 - idx);
        }
    }

    ImagePerceptualHash::from_hash_lo(image_id, PHASH_ALGORITHM, hash)
}

#[cfg(test)]
pub fn hamming_distance(a: &ImagePerceptualHash, b: &ImagePerceptualHash) -> u32 {
    hamming_distance_parts(a.hash_hi, a.hash_lo, b.hash_hi, b.hash_lo)
}

pub fn hamming_distance_parts(a_hi: i64, a_lo: i64, b_hi: i64, b_lo: i64) -> u32 {
    ((a_hi as u64) ^ (b_hi as u64)).count_ones() + ((a_lo as u64) ^ (b_lo as u64)).count_ones()
}

fn dct_coefficient(pixels: &[[f64; PHASH_SIZE]; PHASH_SIZE], u: usize, v: usize) -> f64 {
    let mut sum = 0.0;
    for (y, row) in pixels.iter().enumerate().take(PHASH_SIZE) {
        for (x, pixel) in row.iter().enumerate().take(PHASH_SIZE) {
            let x_cos = (((2 * x + 1) as f64 * u as f64 * PI) / (2.0 * PHASH_SIZE as f64)).cos();
            let y_cos = (((2 * y + 1) as f64 * v as f64 * PI) / (2.0 * PHASH_SIZE as f64)).cos();
            sum += *pixel * x_cos * y_cos;
        }
    }
    alpha(u) * alpha(v) * sum
}

fn alpha(index: usize) -> f64 {
    if index == 0 {
        (1.0 / PHASH_SIZE as f64).sqrt()
    } else {
        (2.0 / PHASH_SIZE as f64).sqrt()
    }
}

fn median_without_dc(coeffs: &[f64; DCT_SIZE * DCT_SIZE]) -> f64 {
    let mut values = coeffs[1..].to_vec();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values[values.len() / 2]
}
