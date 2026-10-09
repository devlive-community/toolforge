use super::*;
use crate::formats::{Format, prepare};
use crate::render::{Style, draw, encode, png};

/// 生成再识别：每种码制都要能被读回来
#[test]
fn reads_back_every_generated_format() {
    let style: Style = serde_json::from_value(serde_json::json!({ "scale": 4 })).unwrap();
    let cases = [
        (Format::Ean13, "690123456789", "6901234567892"),
        (Format::Ean8, "9638507", "96385074"),
        (Format::UpcA, "03600029145", "036000291452"),
        (Format::Code128, "ToolForge-2026", "ToolForge-2026"),
        (Format::Code39, "TOOL-42", "TOOL-42"),
        (Format::Code93, "CODE93", "CODE93"),
        (Format::Itf, "1540014128876", "15400141288763"),
        (Format::Codabar, "A40156B", "A40156B"),
        (Format::DataMatrix, "数据矩阵 DM", "数据矩阵 DM"),
        (Format::Pdf417, "PDF417 text", "PDF417 text"),
        (Format::Aztec, "Aztec ✓", "Aztec ✓"),
    ];
    for (format, input, expected) in cases {
        let prepared = prepare(format, input).unwrap();
        let symbol = encode(format, &prepared.content).unwrap();
        let bytes = png(&draw(format, &symbol, &prepared.content, &style).unwrap()).unwrap();
        let found = decode_bytes(&bytes).unwrap_or_else(|e| panic!("{format:?}: {e:?}"));
        let id = serde_json::to_value(format).unwrap();
        // UPC-A 会被读成 EAN-13（前面补 0），两者都算对
        let hit = found.iter().any(|d| {
            (d.format == id || (format == Format::UpcA && d.format == "ean13"))
                && d.text
                    .ends_with(expected.trim_start_matches('A').trim_end_matches('B'))
        });
        assert!(hit, "{format:?}: {found:?}");
    }
}

#[test]
fn reports_images_without_codes() {
    let blank = GrayImage::from_pixel(200, 120, image::Luma([255]));
    assert_eq!(decode_luma(&blank).unwrap_err().code, "barcode.not_found");
    assert_eq!(
        decode_bytes(b"not an image").unwrap_err().code,
        "barcode.decode_failed"
    );
}

#[test]
fn restores_utf8_read_as_latin1() {
    let latin1: String = "数据".bytes().map(char::from).collect();
    assert_eq!(utf8_from_latin1(&latin1), "数据");
    // 真正的 Latin-1 文字不是合法 UTF-8，保持原样
    assert_eq!(utf8_from_latin1("café"), "café");
    assert_eq!(utf8_from_latin1("plain"), "plain");
}
