//! 编码识别：BOM、无 BOM 的 UTF-16、二进制、UTF-8，其余交给 chardetng 猜测。

use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use serde::Serialize;

/// 判断二进制与 UTF-16 时只看开头这么多字节
const SNIFF: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// BOM 或合法的 UTF-8
    Certain,
    /// 猜测结果，且按该编码解码没有错误
    Likely,
    /// 猜测结果，但解码时有无法识别的字节
    Unsure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detected {
    Binary,
    Text {
        encoding: &'static Encoding,
        bom: bool,
        /// 只含 ASCII 字符，用任何常见编码读都一样
        ascii: bool,
        confidence: Confidence,
    },
}

pub fn bom_of(bytes: &[u8]) -> Option<(&'static Encoding, usize)> {
    Encoding::for_bom(bytes)
}

/// 没有 BOM 的 UTF-16：英文为主的文本每隔一个字节就是 0
fn sniff_utf16(sample: &[u8]) -> Option<&'static Encoding> {
    let pairs = sample.len() / 2;
    if pairs < 4 {
        return None;
    }
    let (mut even, mut odd) = (0, 0);
    for [first, second] in sample.as_chunks::<2>().0 {
        even += usize::from(*first == 0);
        odd += usize::from(*second == 0);
    }
    // 一侧大量为 0、另一侧几乎没有 0
    if odd * 10 >= pairs * 3 && even * 20 <= pairs {
        Some(UTF_16LE)
    } else if even * 10 >= pairs * 3 && odd * 20 <= pairs {
        Some(UTF_16BE)
    } else {
        None
    }
}

/// 无 BOM 的 UTF-16 解码后不应出现控制字符（制表、换行除外）
fn plausible_utf16(bytes: &[u8], encoding: &'static Encoding) -> bool {
    let (text, had_errors) = encoding.decode_without_bom_handling(bytes);
    !had_errors
        && text
            .chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r' | '\u{c}'))
}

pub fn detect(bytes: &[u8]) -> Detected {
    if let Some((encoding, _)) = bom_of(bytes) {
        return Detected::Text {
            encoding,
            bom: true,
            ascii: false,
            confidence: Confidence::Certain,
        };
    }
    let sample = &bytes[..bytes.len().min(SNIFF)];
    if let Some(encoding) = sniff_utf16(sample)
        && bytes.len().is_multiple_of(2)
        && plausible_utf16(bytes, encoding)
    {
        return Detected::Text {
            encoding,
            bom: false,
            ascii: false,
            confidence: Confidence::Likely,
        };
    }
    if sample.contains(&0) {
        return Detected::Binary;
    }
    if bytes.is_ascii() {
        return Detected::Text {
            encoding: UTF_8,
            bom: false,
            ascii: true,
            confidence: Confidence::Certain,
        };
    }
    if std::str::from_utf8(bytes).is_ok() {
        return Detected::Text {
            encoding: UTF_8,
            bom: false,
            ascii: false,
            confidence: Confidence::Certain,
        };
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let encoding = detector.guess(None, false);
    let (_, had_errors) = encoding.decode_without_bom_handling(bytes);
    Detected::Text {
        encoding,
        bom: false,
        ascii: false,
        confidence: if had_errors {
            Confidence::Unsure
        } else {
            Confidence::Likely
        },
    }
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
