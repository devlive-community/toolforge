//! 剪贴板识别：URL 编码、HTML 实体、\\u 转义与 Base64 文本；
//! 标签就是编码方式，打开工具时据此切换到对应的解码模式。

use data_encoding::{BASE64, BASE64URL};
use tf_plugin_api::Detection;

fn has_percent_escape(text: &str) -> bool {
    text.as_bytes()
        .windows(3)
        .any(|w| w[0] == b'%' && w[1].is_ascii_hexdigit() && w[2].is_ascii_hexdigit())
}

fn has_entity(text: &str) -> bool {
    text.match_indices('&').any(|(i, _)| {
        let rest = &text[i + 1..];
        let end = rest.find(';').filter(|&e| (2..=10).contains(&e));
        end.is_some_and(|e| {
            let name = &rest[..e];
            name.strip_prefix('#').map_or_else(
                || name.bytes().all(|b| b.is_ascii_alphabetic()),
                |n| n.bytes().all(|b| b.is_ascii_alphanumeric()),
            )
        })
    })
}

fn has_unicode_escape(text: &str) -> bool {
    text.match_indices("\\u").any(|(i, _)| {
        text.as_bytes()
            .get(i + 2..i + 6)
            .is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit))
    })
}

/// 能解码为可读 UTF-8 文本的 Base64
fn base64_text(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 16
        || !bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=-_".contains(b))
        || bytes.iter().all(u8::is_ascii_hexdigit)
    {
        return false;
    }
    let decoded = BASE64
        .decode(bytes)
        .or_else(|_| BASE64URL.decode(bytes))
        .or_else(|_| data_encoding::BASE64_NOPAD.decode(bytes))
        .or_else(|_| data_encoding::BASE64URL_NOPAD.decode(bytes));
    let Ok(decoded) = decoded else {
        return false;
    };
    let Ok(decoded) = String::from_utf8(decoded) else {
        return false;
    };
    let printable = decoded
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .count();
    printable * 10 >= decoded.chars().count() * 9
}

pub fn detect(text: &str) -> Option<Detection> {
    if has_percent_escape(text) && !text.contains(char::is_whitespace) {
        return Some(Detection::new(75, "url"));
    }
    if has_unicode_escape(text) {
        return Some(Detection::new(70, "unicode"));
    }
    if has_entity(text) {
        return Some(Detection::new(65, "html"));
    }
    let url_safe = text.contains(['-', '_']);
    base64_text(text).then(|| Detection::new(60, if url_safe { "base64url" } else { "base64" }))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
