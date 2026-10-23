use image::Rgba;

use super::*;

#[test]
fn picks_pixels_and_averages() {
    let image = RgbaImage::from_fn(20, 20, |x, _| {
        if x < 10 {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([200, 100, 50, 255])
        }
    });
    let one = pick(&image, 15, 5, 0).unwrap();
    assert_eq!((one.rgb, one.alpha), ([200, 100, 50], 255));
    assert_eq!(one.formats.hex, "#c86432");
    // 跨越边界取平均：左侧 1 列黑色、右侧 2 列彩色
    let mixed = pick(&image, 10, 10, 1).unwrap();
    assert_eq!(mixed.rgb, [133, 67, 33]);
    assert_eq!(
        pick(&image, 20, 0, 0).unwrap_err().code,
        "palette.out_of_bounds"
    );
}

#[test]
fn builds_the_loupe() {
    let image = RgbaImage::from_pixel(5, 5, Rgba([1, 2, 3, 255]));
    let corner = pick(&image, 0, 0, 0).unwrap();
    assert_eq!(corner.loupe.len(), (LOUPE * LOUPE) as usize);
    let center = (LOUPE * LOUPE / 2) as usize;
    assert_eq!(corner.loupe[center].as_deref(), Some("#010203"));
    assert_eq!(corner.loupe[0], None, "outside the image");
}
