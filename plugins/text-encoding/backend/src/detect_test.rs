use encoding_rs::{BIG5, GBK, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};

use super::*;

fn text(encoding: &'static Encoding, bom: bool, confidence: Confidence) -> Detected {
    Detected::Text {
        encoding,
        bom,
        ascii: false,
        confidence,
    }
}

fn utf16(s: &str, little: bool) -> Vec<u8> {
    s.encode_utf16()
        .flat_map(|u| {
            if little {
                u.to_le_bytes()
            } else {
                u.to_be_bytes()
            }
        })
        .collect()
}

#[test]
fn honours_byte_order_marks() {
    assert_eq!(
        detect(b"\xef\xbb\xbfhello"),
        text(UTF_8, true, Confidence::Certain)
    );
    let mut le = vec![0xff, 0xfe];
    le.extend(utf16("你好", true));
    assert_eq!(detect(&le), text(UTF_16LE, true, Confidence::Certain));
    let mut be = vec![0xfe, 0xff];
    be.extend(utf16("hi", false));
    assert_eq!(detect(&be), text(UTF_16BE, true, Confidence::Certain));
}

#[test]
fn recognises_utf16_without_a_bom() {
    let sample = "Hello, world!\r\nThis is a UTF-16 file.\r\n";
    assert_eq!(
        detect(&utf16(sample, true)),
        text(UTF_16LE, false, Confidence::Likely)
    );
    assert_eq!(
        detect(&utf16(sample, false)),
        text(UTF_16BE, false, Confidence::Likely)
    );
}

#[test]
fn separates_ascii_utf8_and_binary() {
    assert_eq!(
        detect(b"plain ascii\n"),
        Detected::Text {
            encoding: UTF_8,
            bom: false,
            ascii: true,
            confidence: Confidence::Certain
        }
    );
    assert_eq!(
        detect("中文 UTF-8".as_bytes()),
        text(UTF_8, false, Confidence::Certain)
    );
    assert_eq!(
        detect(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0"),
        Detected::Binary
    );
    assert_eq!(
        detect(b""),
        Detected::Text {
            encoding: UTF_8,
            bom: false,
            ascii: true,
            confidence: Confidence::Certain
        }
    );
}

#[test]
fn guesses_legacy_cjk_encodings() {
    let zh = "这是一个用简体中文写的配置文件，里面有数据库地址、用户名和密码等设置。\n";
    let gbk = GBK.encode(&zh.repeat(3)).0.into_owned();
    assert_eq!(detect(&gbk), text(GBK, false, Confidence::Likely));

    let tw = "這是一個用繁體中文寫的設定檔案，裡面有資料庫位址、使用者名稱與密碼等設定。\n";
    let big5 = BIG5.encode(&tw.repeat(3)).0.into_owned();
    assert_eq!(detect(&big5), text(BIG5, false, Confidence::Likely));

    let ja =
        "これは日本語で書かれた設定ファイルです。データベースのアドレスとユーザー名があります。\n";
    let sjis = SHIFT_JIS.encode(&ja.repeat(3)).0.into_owned();
    assert_eq!(detect(&sjis), text(SHIFT_JIS, false, Confidence::Likely));
}
