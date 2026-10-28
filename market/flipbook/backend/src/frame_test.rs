use super::*;

#[test]
fn blank_text_is_a_transparent_canvas_and_round_trips() {
    let image = decode("", 240, 180).expect("blank");
    assert_eq!(image.dimensions(), (240, 180));
    assert!(image.pixels().all(|p| p[3] == 0));
    assert_eq!(encode(&image).unwrap(), "", "全透明帧不占存储");
}

#[test]
fn drawn_pixels_survive_encode_and_decode() {
    let mut image = RgbaImage::new(240, 180);
    image.put_pixel(3, 4, image::Rgba([10, 20, 30, 255]));
    let text = encode(&image).unwrap();
    assert!(!text.is_empty());
    assert_eq!(decode(&text, 240, 180).unwrap(), image);
}

#[test]
fn frames_of_another_size_are_rejected() {
    let mut image = RgbaImage::new(320, 240);
    image.put_pixel(0, 0, image::Rgba([1, 2, 3, 255]));
    let text = encode(&image).unwrap();
    assert_eq!(
        decode(&text, 240, 180).unwrap_err().code,
        "flipbook.bad_frame"
    );
}

#[test]
fn only_preset_sizes_are_allowed() {
    assert!(check_size(640, 480).is_ok());
    assert_eq!(
        check_size(4000, 4000).unwrap_err().code,
        "flipbook.bad_size"
    );
}

#[test]
fn colors_must_be_six_digit_hex() {
    assert_eq!(parse_color("#FF8000").unwrap(), [255, 128, 0]);
    assert!(parse_color("#fff").is_err());
    assert!(parse_color("red").is_err());
}

#[test]
fn blending_half_alpha_over_transparent_keeps_color() {
    let mut p = image::Rgba([0, 0, 0, 0]);
    blend(&mut p, [200, 100, 50], 0.5);
    assert_eq!(p, image::Rgba([200, 100, 50, 128]));
}
