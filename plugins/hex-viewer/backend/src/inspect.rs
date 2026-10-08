//! 数据检查器：把光标处的字节按各种类型解读。

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
    out
}

#[cfg(test)]
#[path = "inspect_test.rs"]
mod tests;
