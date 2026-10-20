use super::*;
use image::Rgba;

fn work() -> RgbaImage {
    // 2x2：左上白、右上红、左下蓝、右下绿
    let mut image = RgbaImage::new(2, 2);
    image.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
    image.put_pixel(1, 0, Rgba([255, 0, 0, 255]));
    image.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
    image.put_pixel(1, 1, Rgba([0, 255, 0, 255]));
    image
}

#[test]
fn samples_triangle_average_color() {
    // 覆盖左列两个像素（白 + 蓝）的平均
    let colors = colors(
        &work(),
        &[[0.0, 0.0], [1.0, 1.0], [0.0, 2.0]],
        &[[0, 1, 2]],
        0,
    );
    assert_eq!(colors.len(), 1);
    assert_eq!(colors[0], "#7f7fff");
}

#[test]
fn brightness_shifts_colors() {
    let bright = colors(
        &work(),
        &[[0.0, 0.0], [1.0, 1.0], [0.0, 2.0]],
        &[[0, 1, 2]],
        100,
    );
    assert_eq!(bright[0], "#ffffff");
    let dark = colors(
        &work(),
        &[[0.0, 0.0], [1.0, 1.0], [0.0, 2.0]],
        &[[0, 1, 2]],
        -100,
    );
    assert_eq!(dark[0], "#000000");
}

#[test]
fn degenerate_triangle_falls_back_to_vertex_pixel() {
    // 零面积三角（三点共线）：取顶点像素
    let colors = colors(
        &work(),
        &[[0.0, 0.0], [0.5, 0.0], [1.0, 0.0]],
        &[[0, 1, 2]],
        0,
    );
    assert_eq!(colors[0], "#ffffff");
}

#[test]
fn transparent_pixels_blend_with_white() {
    let mut image = RgbaImage::new(2, 2);
    for y in 0..2 {
        for x in 0..2 {
            image.put_pixel(x, y, Rgba([0, 0, 0, 128]));
        }
    }
    // alpha=128 与白底合成 ≈ 128
    let colors = colors(
        &image,
        &[[0.0, 0.0], [1.0, 1.0], [0.0, 2.0]],
        &[[0, 1, 2]],
        0,
    );
    let value = u32::from_str_radix(&colors[0][1..], 16).unwrap();
    let [r, g, b] = [(value >> 16) as u8, (value >> 8) as u8, value as u8];
    assert!(r.abs_diff(128) <= 1 && g.abs_diff(128) <= 1 && b.abs_diff(128) <= 1);
}

#[test]
fn parses_colors() {
    assert_eq!(parse_color("#ff8000"), Some([255, 128, 0]));
    assert_eq!(parse_color("ff8000"), Some([255, 128, 0]));
    assert_eq!(parse_color("#f00"), None);
    assert_eq!(parse_color("#zzzzzz"), None);
}
