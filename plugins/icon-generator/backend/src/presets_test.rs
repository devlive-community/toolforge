use super::*;

#[test]
fn ios_sizes_match_points_and_scales() {
    assert_eq!(ios_pixels("83.5", 2), 167);
    assert_eq!(ios_pixels("20", 3), 60);
    assert_eq!(ios_pixels("1024", 1), 1024);
}

#[test]
fn writes_multi_size_ico_files() {
    let images: Vec<RgbaImage> = [16, 32, 256]
        .iter()
        .map(|&s| RgbaImage::from_pixel(s, s, image::Rgba([0, 128, 255, 255])))
        .collect();
    let bytes = ico(&images).unwrap();
    // 头部：保留 0、类型 1、数量 3；256 像素在目录中记为 0
    assert_eq!(&bytes[..6], &[0, 0, 1, 0, 3, 0]);
    assert_eq!(bytes[6], 16);
    assert_eq!(bytes[6 + 32], 0);
    let decoded = image::load_from_memory_with_format(&bytes, ImageFormat::Ico).unwrap();
    assert_eq!(decoded.width(), 256);
}

#[test]
fn writes_icns_with_png_entries() {
    let mut asked = Vec::new();
    let bytes = icns(&mut |size| {
        asked.push(size);
        png_bytes(&RgbaImage::new(size, size))
    });
    assert_eq!(&bytes[..4], b"icns");
    assert_eq!(
        u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len()
    );
    // 相同尺寸只编码一次
    asked.sort();
    assert_eq!(asked, [16, 32, 64, 128, 256, 512, 1024]);
    // 逐条解析：类型 + 长度 + PNG
    let mut at = 8;
    let mut kinds = Vec::new();
    while at < bytes.len() {
        let kind = std::str::from_utf8(&bytes[at..at + 4]).unwrap().to_owned();
        let len = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        assert_eq!(&bytes[at + 8..at + 12], b"\x89PNG");
        kinds.push(kind);
        at += len;
    }
    assert_eq!(at, bytes.len());
    assert_eq!(kinds.len(), ICNS_ENTRIES.len());
}
