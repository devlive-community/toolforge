use super::*;

fn temp(name: &str, bytes: &[u8]) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tfp-icon-source-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn renders_svg_at_the_requested_size() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="10" height="10" fill="#ff0000"/></svg>"##;
    let source = load(&temp("logo.svg", svg), 200).unwrap();
    assert!(source.svg);
    assert_eq!((source.width, source.height), (20, 10));
    assert_eq!(source.image.dimensions(), (200, 100));
    assert_eq!(source.image.get_pixel(50, 50).0, [255, 0, 0, 255]);
    assert_eq!(source.image.get_pixel(150, 50).0[3], 0);
}

#[test]
fn decodes_raster_images() {
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 4]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let source = load(&temp("a.png", &png), 1024).unwrap();
    assert!(!source.svg);
    assert_eq!(source.image.dimensions(), (3, 2));
}

#[test]
fn reports_unreadable_files() {
    assert_eq!(
        load(&temp("bad.png", b"nope"), 64).unwrap_err().code,
        "icon.unsupported_image"
    );
    assert_eq!(
        load(&temp("bad.svg", b"<svg"), 64).unwrap_err().code,
        "icon.invalid_svg"
    );
    assert_eq!(
        load("/no/such/file.png", 64).unwrap_err().code,
        "fs.not_found"
    );
}
