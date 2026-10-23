use super::*;
use crate::test_support::workspace;

fn write_image(path: &Path, format: ImageFormat, rgba: bool, width: u32, height: u32) {
    let image = if rgba {
        DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, _| {
            image::Rgba([200, 30, 30, if x < width / 2 { 0 } else { 255 }])
        }))
    } else {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([x as u8, y as u8, 120])
        }))
    };
    image.save_with_format(path, format).unwrap();
}

#[test]
fn lays_out_pages() {
    assert_eq!(
        layout(PageSize::Fit, 0.0, 800, 600),
        ((600.0, 450.0), (0.0, 0.0, 600.0, 450.0))
    );
    let ((w, h), (x, y, dw, dh)) = layout(PageSize::A4, 36.0, 4000, 3000);
    assert_eq!(
        (w, h),
        (841.89, 595.28),
        "landscape images get landscape pages"
    );
    assert!((dw - (w - 72.0)).abs() < 0.01 || (dh - (h - 72.0)).abs() < 0.01);
    assert!(x >= 36.0 - 0.01 && y >= 36.0 - 0.01);
    // 小图不放大，居中
    let (_, (x, y, dw, dh)) = layout(PageSize::Letter, 0.0, 100, 100);
    assert_eq!((dw, dh), (75.0, 75.0));
    assert!((x - (612.0 - 75.0) / 2.0).abs() < 0.01 && (y - (792.0 - 75.0) / 2.0).abs() < 0.01);
}

#[test]
fn embeds_jpeg_as_is_and_other_formats_losslessly() {
    let dir = workspace("images");
    let jpeg = dir.join("photo.jpg");
    let png = dir.join("logo.png");
    write_image(&jpeg, ImageFormat::Jpeg, false, 64, 48);
    write_image(&png, ImageFormat::Png, true, 32, 32);
    let args = Args {
        paths: vec![
            jpeg.to_string_lossy().into_owned(),
            png.to_string_lossy().into_owned(),
        ],
        output: String::new(),
        page_size: PageSize::A4,
        margin: 20.0,
    };
    let mut doc = images_to_pdf(&args, |_| {}, || false).unwrap();
    let output = dir.join("images.pdf");
    doc.save(&output).unwrap();
    let reloaded = Document::load(&output).unwrap();
    assert_eq!(reloaded.get_pages().len(), 2);
    let images: Vec<&Stream> = reloaded
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.dict
                .get(b"Subtype")
                .and_then(Object::as_name)
                .is_ok_and(|n| n == b"Image")
        })
        .collect();
    let jpeg_stream = images
        .iter()
        .find(|s| {
            s.dict
                .get(b"Filter")
                .and_then(Object::as_name)
                .is_ok_and(|f| f == b"DCTDecode")
        })
        .unwrap();
    assert_eq!(
        jpeg_stream.content,
        std::fs::read(&jpeg).unwrap(),
        "the jpeg is embedded unchanged"
    );
    assert!(
        images.iter().any(|s| s.dict.has(b"SMask")),
        "transparency becomes a soft mask"
    );
    assert_eq!(images.len(), 3);
}

#[test]
fn rejects_missing_and_unsupported_images() {
    let dir = workspace("bad-images");
    let text = dir.join("notes.txt");
    std::fs::write(&text, b"hello").unwrap();
    let args = |paths: Vec<String>| Args {
        paths,
        output: String::new(),
        page_size: PageSize::Fit,
        margin: 0.0,
    };
    assert_eq!(
        images_to_pdf(&args(vec![]), |_| {}, || false)
            .unwrap_err()
            .code,
        "pdf.no_images"
    );
    let err = images_to_pdf(
        &args(vec![text.to_string_lossy().into_owned()]),
        |_| {},
        || false,
    )
    .unwrap_err();
    assert_eq!(err.code, "pdf.image_unsupported");
}
