//! 数据检查器：把光标处的字节按各种类型解读。

use data_encoding::{BASE32, BASE64};
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq)]
pub struct Value {
    /// 类型标识，前端翻译名称
    pub kind: &'static str,
    pub value: String,
}

fn take<const N: usize>(bytes: &[u8], little: bool) -> Option<[u8; N]> {
    let mut out: [u8; N] = bytes.get(..N)?.try_into().ok()?;
    if !little {
        out.reverse();
    }
    Some(out)
}

/// 极大或极小的浮点数用科学计数法，避免一长串 0
fn float<T: std::fmt::Display + std::fmt::LowerExp + Into<f64> + Copy>(value: T) -> String {
    let abs = value.into().abs();
    if abs != 0.0 && abs.is_finite() && !(1e-6..1e15).contains(&abs) {
        format!("{value:e}")
    } else {
        value.to_string()
    }
}

fn time(seconds: i64, nanos: i32) -> Option<String> {
    jiff::Timestamp::new(seconds, nanos)
        .ok()
        .map(|t| t.strftime("%Y-%m-%d %H:%M:%S%.f UTC").to_string())
}

/// 检查器读取的字节数（C 字符串、Base64 片段需要更多字节）
pub const READ_BYTES: u64 = 256;

/// protobuf varint（LEB128，最多 10 字节）：返回值与占用字节数
pub fn varint(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (index, byte) in bytes.iter().take(10).enumerate() {
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

/// 压缩 BCD：每个半字节一个十进制数字，遇到非法半字节停止
fn bcd(bytes: &[u8]) -> Option<String> {
    let mut digits = String::new();
    for byte in bytes.iter().take(8) {
        let (high, low) = (byte >> 4, byte & 0x0f);
        if high > 9 {
            break;
        }
        digits.push(char::from(b'0' + high));
        if low > 9 {
            break;
        }
        digits.push(char::from(b'0' + low));
    }
    (!digits.is_empty()).then_some(digits)
}

/// UUID：大端为 RFC 9562 字节序；小端按 Windows GUID（前三段小端）解读
fn uuid(bytes: &[u8], little: bool) -> Option<String> {
    let raw: [u8; 16] = bytes.get(..16)?.try_into().ok()?;
    let id = if little {
        uuid::Uuid::from_bytes_le(raw)
    } else {
        uuid::Uuid::from_bytes(raw)
    };
    let mut text = id.hyphenated().to_string();
    let version = id.get_version_num();
    if id.get_variant() == uuid::Variant::RFC4122 && (1..=8).contains(&version) {
        text.push_str(&format!(" · v{version}"));
        if let Some(ts) = id.get_timestamp() {
            let (secs, nanos) = ts.to_unix();
            if let Some(t) = time(secs as i64, nanos as i32) {
                text.push_str(&format!(" · {t}"));
            }
        }
    }
    Some(text)
}

fn rgb565(value: u16) -> String {
    let r = ((value >> 11) & 0x1f) as u32;
    let g = ((value >> 5) & 0x3f) as u32;
    let b = (value & 0x1f) as u32;
    format!(
        "#{:02X}{:02X}{:02X}",
        r * 255 / 31,
        g * 255 / 63,
        b * 255 / 31
    )
}

/// 以 \0 结尾的字符串（在读取范围内找到结尾才显示）
fn cstring(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|b| *b == 0)?;
    let text = std::str::from_utf8(&bytes[..end]).ok()?;
    if text.chars().any(|c| c.is_control() && c != '\t') {
        return None;
    }
    Some(format!("\"{}\" ({end} B)", text.escape_debug()))
}

/// 解码后的预览：可打印 UTF-8 显示文本，否则显示十六进制
fn preview(decoded: &[u8]) -> String {
    match std::str::from_utf8(decoded) {
        Ok(text) if !text.chars().any(|c| c.is_control() && !c.is_whitespace()) => {
            format!("\"{}\"", text.escape_debug())
        }
        _ => decoded
            .iter()
            .take(24)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// 从光标开始的 Base64 / Base32 片段（至少 8 个字符，按 4 / 8 字符对齐后解码）
fn encoded(bytes: &[u8], base32: bool) -> Option<String> {
    let valid = |b: &u8| {
        if base32 {
            b.is_ascii_uppercase() || (b'2'..=b'7').contains(b) || *b == b'='
        } else {
            b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')
        }
    };
    let run = bytes.iter().take_while(|b| valid(b)).count();
    let block = if base32 { 8 } else { 4 };
    let len = run - run % block;
    if len < 8 {
        return None;
    }
    let text = &bytes[..len];
    let decoded = if base32 {
        BASE32.decode(text).ok()?
    } else {
        BASE64.decode(text).ok()?
    };
    if decoded.is_empty() {
        return None;
    }
    Some(format!(
        "{} → {} ({} B)",
        String::from_utf8_lossy(&text[..len.min(24)]),
        preview(&decoded),
        decoded.len()
    ))
}

/// 解读从光标开始的字节；字节不够的类型不出现
pub fn values(bytes: &[u8], little: bool) -> Vec<Value> {
    let mut out = Vec::new();
    let mut push = |kind: &'static str, value: Option<String>| {
        if let Some(value) = value {
            out.push(Value { kind, value });
        }
    };
    push("binary", bytes.first().map(|b| format!("{b:08b}")));
    push("u8", bytes.first().map(|b| b.to_string()));
    push("i8", bytes.first().map(|&b| (b as i8).to_string()));
    push(
        "u16",
        take(bytes, little).map(|b| u16::from_le_bytes(b).to_string()),
    );
    push(
        "i16",
        take(bytes, little).map(|b| i16::from_le_bytes(b).to_string()),
    );
    push(
        "u32",
        take(bytes, little).map(|b| u32::from_le_bytes(b).to_string()),
    );
    push(
        "i32",
        take(bytes, little).map(|b| i32::from_le_bytes(b).to_string()),
    );
    push(
        "u64",
        take(bytes, little).map(|b| u64::from_le_bytes(b).to_string()),
    );
    push(
        "i64",
        take(bytes, little).map(|b| i64::from_le_bytes(b).to_string()),
    );
    push(
        "f32",
        take(bytes, little).map(|b| float(f32::from_le_bytes(b))),
    );
    push(
        "f64",
        take(bytes, little).map(|b| float(f64::from_le_bytes(b))),
    );
    push(
        "utf8",
        bytes.first().and_then(|&first| {
            let len = match first {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf7 => 4,
                _ => return None,
            };
            let c = std::str::from_utf8(bytes.get(..len)?)
                .ok()?
                .chars()
                .next()?;
            Some(format!("{} U+{:04X}", c.escape_debug(), c as u32))
        }),
    );
    push(
        "unix32",
        take(bytes, little).and_then(|b| time(u32::from_le_bytes(b) as i64, 0)),
    );
    push(
        "unixMs",
        take(bytes, little).and_then(|b| {
            let ms = i64::from_le_bytes(b);
            time(
                ms.div_euclid(1000),
                (ms.rem_euclid(1000) * 1_000_000) as i32,
            )
        }),
    );
    push(
        "fixed8_8",
        take(bytes, little).map(|b| float(i16::from_le_bytes(b) as f64 / 256.0)),
    );
    push(
        "fixed16_16",
        take(bytes, little).map(|b| float(i32::from_le_bytes(b) as f64 / 65536.0)),
    );
    let var = varint(bytes);
    push(
        "varint",
        var.map(|(value, len)| format!("{value} ({len} B)")),
    );
    push(
        "zigzag",
        var.map(|(value, _)| (((value >> 1) as i64) ^ -((value & 1) as i64)).to_string()),
    );
    push("bcd", bcd(bytes));
    push("uuid", uuid(bytes, little));
    push(
        "rgba",
        bytes
            .get(..4)
            .map(|b| format!("#{:02X}{:02X}{:02X}{:02X}", b[0], b[1], b[2], b[3])),
    );
    push(
        "rgb565",
        take(bytes, little).map(|b| rgb565(u16::from_le_bytes(b))),
    );
    push("cstring", cstring(bytes));
    push("base64", encoded(bytes, false));
    push("base32", encoded(bytes, true));
    out
}

#[cfg(test)]
#[path = "inspect_test.rs"]
mod tests;
