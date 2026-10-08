use encoding_rs::{BIG5, GB18030, GBK, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};

use super::*;

fn target(encoding: &'static Encoding) -> Target {
    Target {
        encoding,
        bom: false,
        newline: NewLine::Keep,
    }
}

#[test]
fn looks_up_supported_encodings_by_label() {
    assert_eq!(encoding("utf8").unwrap(), UTF_8);
    assert_eq!(encoding("GB2312").unwrap(), GBK);
    assert_eq!(encoding("gb18030").unwrap(), GB18030);
    assert_eq!(encoding("sjis").unwrap(), SHIFT_JIS);
    assert_eq!(encoding("utf-16").unwrap(), UTF_16LE);
    assert_eq!(encoding("nope").unwrap_err().code, "encoding.unsupported");
    // encoding_rs 认识但不提供的编码
    assert_eq!(
        encoding("ISO-2022-JP").unwrap_err().code,
        "encoding.unsupported"
    );
    for name in ENCODINGS {
        assert_eq!(encoding(name).unwrap().name(), *name);
    }
}

#[test]
fn classifies_line_endings() {
    assert_eq!(line_endings("one line"), LineEndings::None);
    assert_eq!(line_endings("a\nb\n"), LineEndings::Lf);
    assert_eq!(line_endings("a\r\nb\r\n"), LineEndings::Crlf);
    assert_eq!(line_endings("a\rb\r"), LineEndings::Cr);
    assert_eq!(line_endings("a\r\nb\n"), LineEndings::Mixed);
}

#[test]
fn converts_gbk_to_utf8_and_back() {
    let text = "数据库=本地\n用户=管理员\n";
    let gbk = GBK.encode(text).0.into_owned();
    let utf8 = transcode(&gbk, GBK, target(UTF_8)).unwrap().unwrap();
    assert_eq!(utf8, text.as_bytes());
    let back = transcode(&utf8, UTF_8, target(GBK)).unwrap().unwrap();
    assert_eq!(back, gbk);
    // 已经是目标编码时没有变化
    assert_eq!(transcode(&utf8, UTF_8, target(UTF_8)).unwrap(), None);
}

#[test]
fn writes_and_strips_byte_order_marks() {
    let with_bom = Target {
        bom: true,
        ..target(UTF_8)
    };
    let out = transcode("héllo".as_bytes(), UTF_8, with_bom)
        .unwrap()
        .unwrap();
    assert_eq!(out, "\u{feff}héllo".as_bytes());
    // 去掉 BOM
    assert_eq!(
        transcode(&out, UTF_8, target(UTF_8)).unwrap().unwrap(),
        "héllo".as_bytes()
    );
    // 已有 BOM 且保留时没有变化
    assert_eq!(transcode(&out, UTF_8, with_bom).unwrap(), None);
}

#[test]
fn encodes_utf16_in_both_byte_orders() {
    let le = transcode(
        "A中".as_bytes(),
        UTF_8,
        Target {
            bom: true,
            ..target(UTF_16LE)
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(le, [0xff, 0xfe, 0x41, 0x00, 0x2d, 0x4e]);
    let be = transcode("A中".as_bytes(), UTF_8, target(UTF_16BE))
        .unwrap()
        .unwrap();
    assert_eq!(be, [0x00, 0x41, 0x4e, 0x2d]);
    // 再转回 UTF-8
    assert_eq!(
        transcode(&le, UTF_16LE, target(UTF_8)).unwrap().unwrap(),
        "A中".as_bytes()
    );
}

#[test]
fn converts_line_endings() {
    let crlf = Target {
        newline: NewLine::Crlf,
        ..target(UTF_8)
    };
    let lf = Target {
        newline: NewLine::Lf,
        ..target(UTF_8)
    };
    assert_eq!(
        transcode(b"a\nb\r\nc\rd", UTF_8, crlf).unwrap().unwrap(),
        b"a\r\nb\r\nc\r\nd"
    );
    assert_eq!(
        transcode(b"a\r\nb\rc\n", UTF_8, lf).unwrap().unwrap(),
        b"a\nb\nc\n"
    );
    assert_eq!(transcode(b"a\nb\n", UTF_8, lf).unwrap(), None);
}

#[test]
fn reports_characters_the_target_cannot_hold() {
    let error = transcode(
        "第一行\n含有 emoji 😀 的行\n".as_bytes(),
        UTF_8,
        target(GBK),
    )
    .unwrap_err();
    assert_eq!(error.code, "encoding.unmappable");
    assert_eq!(error.params["char"], "😀");
    assert_eq!(error.params["code"], "U+1F600");
    assert_eq!(error.params["line"], 2);
    // GB18030 能表示全部 Unicode
    assert!(transcode("😀".as_bytes(), UTF_8, target(GB18030)).is_ok());
    // 简体字在 Big5 中没有（与 在 encoding_rs 的 Big5 里有）
    let error = transcode("繁體与简体".as_bytes(), UTF_8, target(BIG5)).unwrap_err();
    assert_eq!(error.params["char"], "简");
}

#[test]
fn reports_bytes_that_are_not_in_the_source_encoding() {
    let error = transcode(b"ok\nbad \xff\xfe here\n", UTF_8, target(GBK)).unwrap_err();
    assert_eq!(error.code, "encoding.malformed");
    assert_eq!(error.params["encoding"], "UTF-8");
    assert_eq!(error.params["line"], 2);
}
