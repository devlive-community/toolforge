use super::*;

#[test]
fn converts_through_oklab_and_back() {
    let white = rgb_to_oklab([255, 255, 255]);
    assert!((white[0] - 1.0).abs() < 1e-3 && white[1].abs() < 1e-3 && white[2].abs() < 1e-3);
    for rgb in [
        [0, 0, 0],
        [255, 0, 0],
        [12, 200, 77],
        [128, 128, 128],
        [250, 240, 10],
    ] {
        assert_eq!(oklab_to_rgb(rgb_to_oklab(rgb)), rgb, "{rgb:?}");
    }
}

#[test]
fn writes_every_format() {
    let red = formats([255, 0, 0]);
    assert_eq!(red.hex, "#ff0000");
    assert_eq!(red.rgb, "rgb(255 0 0)");
    assert_eq!(red.hsl, "hsl(0 100% 50%)");
    assert!(
        red.oklch.starts_with("oklch(62.8% 0.258 29.2"),
        "{}",
        red.oklch
    );
    let black = formats([0, 0, 0]);
    assert_eq!((black.contrast_white, black.contrast_black), (21.0, 1.0));
    assert_eq!(black.text, "light");
    assert_eq!(formats([255, 255, 255]).text, "dark");
    assert_eq!(formats([128, 128, 128]).hsl, "hsl(0 0% 50%)");
}
