use image::Rgba;

use super::*;

/// 左半红色、四分之一蓝色、四分之一白色
fn blocks() -> RgbaImage {
    RgbaImage::from_fn(400, 100, |x, _| match x {
        0..200 => Rgba([220, 30, 40, 255]),
        200..300 => Rgba([20, 60, 200, 255]),
        _ => Rgba([255, 255, 255, 255]),
    })
}

#[test]
fn finds_dominant_colors_with_shares() {
    let swatches = extract(&blocks(), 3);
    assert_eq!(swatches.len(), 3);
    assert_eq!(swatches[0].formats.hex, "#dc1e28");
    assert!(
        (swatches[0].share - 0.5).abs() < 0.02,
        "{}",
        swatches[0].share
    );
    let rest: Vec<&str> = swatches[1..]
        .iter()
        .map(|s| s.formats.hex.as_str())
        .collect();
    assert!(
        rest.contains(&"#143cc8") && rest.contains(&"#ffffff"),
        "{rest:?}"
    );
    // 结果可复现
    assert_eq!(extract(&blocks(), 3), swatches);
}

#[test]
fn merges_duplicates_and_skips_transparency() {
    // 只有一种颜色时多余的中心被合并
    let flat = RgbaImage::from_pixel(50, 50, Rgba([10, 120, 90, 255]));
    let swatches = extract(&flat, 6);
    assert_eq!(swatches.len(), 1);
    assert_eq!(swatches[0].share, 1.0);
    // 透明背景不参与
    let logo = RgbaImage::from_fn(100, 100, |x, _| {
        if x < 20 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let swatches = extract(&logo, 4);
    assert_eq!(swatches[0].formats.hex, "#ff0000");
    assert_eq!(swatches.len(), 1);
    assert!(extract(&RgbaImage::new(10, 10), 4).is_empty());
}
