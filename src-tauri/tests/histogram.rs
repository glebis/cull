use cull_lib::preview::histogram::{compute_image_histogram, HistogramSource};
use image::{DynamicImage, Rgb, RgbImage, Rgba, RgbaImage};

#[test]
fn computes_rgb_histogram_bins_from_real_pixels() {
    let mut pixels = RgbImage::new(2, 1);
    pixels.put_pixel(0, 0, Rgb([255, 0, 0]));
    pixels.put_pixel(1, 0, Rgb([255, 255, 255]));
    let image = DynamicImage::ImageRgb8(pixels);

    let histogram = compute_image_histogram("img-1", &image, HistogramSource::Original);

    assert_eq!(histogram.image_id, "img-1");
    assert_eq!(histogram.source, HistogramSource::Original);
    assert_eq!(histogram.pixel_count, 2);
    assert_eq!(histogram.red[255], 2);
    assert_eq!(histogram.green[0], 1);
    assert_eq!(histogram.green[255], 1);
    assert_eq!(histogram.blue[0], 1);
    assert_eq!(histogram.blue[255], 1);
}

#[test]
fn histogram_pixel_count_tracks_binned_opaque_pixels() {
    let mut pixels = RgbaImage::new(2, 1);
    pixels.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
    pixels.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
    let image = DynamicImage::ImageRgba8(pixels);

    let histogram = compute_image_histogram("img-transparent", &image, HistogramSource::Original);

    assert_eq!(histogram.pixel_count, 1);
    assert_eq!(histogram.luma[0], 0);
    assert_eq!(histogram.luma[255], 1);
}
