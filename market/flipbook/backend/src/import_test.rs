use super::*;
use image::{ImageFormat, Rgba};

fn png(image: &RgbaImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn wide_picture_is_fitted_and_centered_vertically() {
    let picture = RgbaImage::from_pixel(400, 100, Rgba([0, 200, 0, 255]));
    let decoded = decode(&png(&picture)).unwrap();
    let mut frame = RgbaImage::new(240, 180);
    place(&mut frame, &decoded);
    // 400×100 缩放到 240×60，上下各留 60 像素
    assert_eq!(frame.get_pixel(120, 90)[1], 200);
    assert_eq!(frame.get_pixel(120, 30)[3], 0);
    assert_eq!(frame.get_pixel(120, 150)[3], 0);
}

#[test]
fn import_keeps_existing_drawing_under_transparent_areas() {
    let mut picture = RgbaImage::new(240, 180);
    picture.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
    let decoded = decode(&png(&picture)).unwrap();
    let mut frame = RgbaImage::from_pixel(240, 180, Rgba([9, 9, 9, 255]));
    place(&mut frame, &decoded);
    assert_eq!(frame.get_pixel(100, 100), &Rgba([9, 9, 9, 255]));
}

#[test]
fn garbage_bytes_are_rejected() {
    assert_eq!(
        decode(b"not an image").unwrap_err().code,
        "flipbook.bad_image"
    );
}
