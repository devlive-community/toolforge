use super::*;
use crate::backup::check_cover as check_photo;

fn encode(image: &DynamicImage, format: image::ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, format).unwrap();
    out.into_inner()
}

#[test]
fn large_noisy_photo_is_shrunk_into_budget() {
    let mut img = image::RgbImage::new(2400, 1600);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgb([
            (x * 7 % 256) as u8,
            (y * 13 % 256) as u8,
            ((x ^ y) % 256) as u8,
        ]);
    }
    let bytes = encode(&DynamicImage::ImageRgb8(img), image::ImageFormat::Png);
    let uri = compress(&bytes).unwrap();
    assert!(uri.len() <= MAX_PHOTO);
    check_photo(&uri).unwrap();
}

#[test]
fn small_pictures_are_not_enlarged_and_transparency_turns_white() {
    let img = image::RgbaImage::new(30, 20);
    let uri = compress(&encode(
        &DynamicImage::ImageRgba8(img),
        image::ImageFormat::Png,
    ))
    .unwrap();
    let body = uri.strip_prefix("data:image/jpeg;base64,").unwrap();
    let decoded = image::load_from_memory(&data_encoding::BASE64.decode(body.as_bytes()).unwrap())
        .unwrap()
        .into_rgb8();
    assert_eq!(decoded.dimensions(), (30, 20));
    assert!(decoded.get_pixel(10, 10)[0] > 240);
}

#[test]
fn garbage_is_rejected() {
    assert_eq!(compress(b"nope").unwrap_err().code, "journal.bad_cover");
}
