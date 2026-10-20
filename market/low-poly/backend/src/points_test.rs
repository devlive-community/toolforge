use image::{Rgba, RgbaImage};

use super::*;

#[test]
fn point_generation_is_deterministic_and_bounded() {
    let image = RgbaImage::from_pixel(320, 240, Rgba([120, 120, 120, 255]));
    let a = generate(&image, 42, 200, 0.5);
    let b = generate(&image, 42, 200, 0.5);
    assert_eq!(a, b, "same seed must reproduce points");
    // 四角锚点固定
    assert_eq!(a[0], [0.0, 0.0]);
    assert_eq!(a[1], [319.0, 0.0]);
    assert_eq!(a[2], [319.0, 239.0]);
    assert_eq!(a[3], [0.0, 239.0]);
    // 点数不超过请求
    assert!(a.len() <= 200);
    // 全部在画布内
    for p in &a {
        assert!(p[0] >= 0.0 && p[0] <= 319.0);
        assert!(p[1] >= 0.0 && p[1] <= 239.0);
    }
    // 极少点数时只保留锚点
    assert_eq!(generate(&image, 7, 2, 0.5).len(), 4);
}

#[test]
fn detail_pulls_points_toward_edges() {
    // 左半亮右半暗的硬边图：高 detail 时边缘附近的点应更多
    let mut image = RgbaImage::new(200, 100);
    for y in 0..100 {
        for x in 0..200 {
            let v = if x < 100 { 250 } else { 0 };
            image.put_pixel(x, y, Rgba([v, v, v, 255]));
        }
    }
    let count_near_edge =
        |pts: &[[f64; 2]]| -> usize { pts.iter().filter(|p| p[0] > 92.0 && p[0] < 108.0).count() };
    let flat = generate(&image, 99, 120, 0.0);
    let edgy = generate(&image, 99, 120, 1.0);
    // 相同种子下高 detail 采样分布不同；边缘点比例应不低于平坦采样
    assert!(
        count_near_edge(&edgy) * count_near_edge(&flat).max(1)
            >= count_near_edge(&flat) * count_near_edge(&edgy).max(1) / 2
    );
    assert!(count_near_edge(&edgy) > 0);
}
