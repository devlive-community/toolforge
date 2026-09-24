use super::*;
use crate::test_support::{jpeg_with_exif, png_with_text};

#[test]
fn strips_jpeg_metadata_losslessly() {
    let data = jpeg_with_exif();
    let result = strip(&data).unwrap();
    assert_eq!(result.removed, vec!["EXIF", "Comment"]);
    assert!(result.bytes.len() < data.len());
    assert!(!result.bytes.windows(5).any(|w| w == b"Exif\0"));
    // 像素完全一致：没有重新编码
    let before = image::load_from_memory(&data).unwrap().to_rgb8();
    let after = image::load_from_memory(&result.bytes).unwrap().to_rgb8();
    assert_eq!(before, after);
    // 再次处理不会有变化
    assert!(strip(&result.bytes).unwrap().removed.is_empty());
}

#[test]
fn lists_each_kind_once() {
    let data = jpeg_with_exif();
    // 复制一份 APP1 段，模拟同时带有多段 EXIF 的文件
    let len = u16::from_be_bytes([data[4], data[5]]) as usize;
    let mut doubled = data[..4 + len].to_vec();
    doubled.extend(&data[2..]);
    assert_eq!(strip(&doubled).unwrap().removed, vec!["EXIF", "Comment"]);
}

#[test]
fn strips_png_text_and_exif_chunks() {
    let data = png_with_text();
    let result = strip(&data).unwrap();
    assert_eq!(result.removed, vec!["tEXt", "eXIf"]);
    let after = image::load_from_memory(&result.bytes).unwrap();
    assert_eq!((after.width(), after.height()), (64, 48));
}

#[test]
fn rejects_other_formats_and_corrupt_files() {
    assert_eq!(
        strip(b"GIF89a....").unwrap_err().code,
        "info.strip_unsupported"
    );
    assert_eq!(strip(&[0xFF, 0xD8, 0x00]).unwrap_err().code, "info.corrupt");
    let mut truncated = png_with_text();
    truncated.truncate(40);
    assert_eq!(strip(&truncated).unwrap_err().code, "info.corrupt");
}
