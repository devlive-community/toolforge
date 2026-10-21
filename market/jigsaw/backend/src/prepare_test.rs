use std::io::Cursor;

use super::*;
use image::{ImageFormat, Rgba};

fn png_data(width: u32, height: u32) -> String {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x * 7 % 256) as u8, (y * 11 % 256) as u8, 128, 255])
    });
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    format!("data:image/png;base64,{}", BASE64.encode(out.get_ref()))
}

#[test]
fn prepare_scales_long_edge_and_hashes_bytes() {
    let data = png_data(1200, 600);
    let input = ImageInput {
        data: Some(data.clone()),
        path: None,
    };
    let out = prepare(&input).unwrap();
    assert_eq!((out.width, out.height), (1200, 600));
    assert!(out.work_data.starts_with("data:image/jpeg;base64,"));
    let again = prepare(&input).unwrap();
    assert_eq!(out.photo_key, again.photo_key, "same bytes, same key");
    let other = prepare(&ImageInput {
        data: Some(png_data(600, 1200)),
        path: None,
    })
    .unwrap();
    assert_ne!(out.photo_key, other.photo_key, "different bytes, other key");
    assert_eq!(hex_len(&out.photo_key), 16);
}

fn hex_len(text: &str) -> usize {
    text.chars().count()
}

#[test]
fn prepare_downscales_large_images() {
    let out = prepare(&ImageInput {
        data: Some(png_data(2400, 1200)),
        path: None,
    })
    .unwrap();
    assert_eq!((out.width, out.height), (1600, 800));
}

#[test]
fn prepare_rejects_bad_input() {
    let error = prepare(&ImageInput {
        data: None,
        path: None,
    })
    .unwrap_err();
    assert_eq!(error.code, "jigsaw.source_required");
    let error = prepare(&ImageInput {
        data: Some("!!!not-base64!!!".to_owned()),
        path: None,
    })
    .unwrap_err();
    assert_eq!(error.code, "jigsaw.bad_data");
}

#[test]
fn flatten_composites_onto_white() {
    let mut image = RgbaImage::new(2, 1);
    image.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
    image.put_pixel(1, 0, Rgba([255, 0, 0, 128]));
    let flat = flatten_to_rgb(&DynamicImage::ImageRgba8(image));
    // 全透明 → 白底；半透明红 → 混白
    assert_eq!(flat.get_pixel(0, 0).0, [255, 255, 255, 255]);
    let red = flat.get_pixel(1, 0).0;
    assert!(red[0] > 200 && red[1] < 150 && red[2] < 150);
}
