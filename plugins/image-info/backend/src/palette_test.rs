use image::{DynamicImage, Rgba, RgbaImage};

use super::*;
use crate::test_support::image;

#[test]
fn finds_dominant_colors_by_share() {
    let (swatches, average) = extract(&DynamicImage::ImageRgb8(image()), 4);
    assert_eq!(swatches.len(), 2);
    assert!(swatches.iter().any(|s| s.hex == "#dc1e1e"), "{swatches:?}");
    assert!(swatches.iter().any(|s| s.hex == "#143cc8"), "{swatches:?}");
    let total: f64 = swatches.iter().map(|s| s.percent).sum();
    assert!((total - 100.0).abs() < 0.2);
    assert!(average.is_some());
}

#[test]
fn ignores_transparent_pixels() {
    let transparent = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 0]));
    assert_eq!(
        extract(&DynamicImage::ImageRgba8(transparent), 4),
        (vec![], None)
    );
}
