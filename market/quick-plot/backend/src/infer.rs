//! 单元格与列的类型推断：数值、时间（统一为毫秒时间戳）或文本。

use std::sync::OnceLock;

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Number,
    Time,
    Text,
}

/// 至少这么大比例的非空单元格能解析，才认定为数值 / 时间列
const RATIO: f64 = 0.9;

/// 本地时区：没有偏移的时间按本地时间解释，刻度也按本地时间标注
pub fn local_zone() -> &'static TimeZone {
    static ZONE: OnceLock<TimeZone> = OnceLock::new();
    ZONE.get_or_init(|| TimeZone::try_system().unwrap_or(TimeZone::UTC))
}

/// 视为缺失值的写法
pub fn is_missing(cell: &str) -> bool {
    matches!(
        cell.trim(),
        "" | "-" | "—" | "null" | "NULL" | "None" | "NaN" | "nan" | "N/A" | "n/a" | "NA"
    )
}

/// 普通十进制或科学计数法，可带正负号与末尾百分号；不接受 inf / nan 与千分位
pub fn number(cell: &str) -> Option<f64> {
    let text = cell.trim();
    let text = text.strip_suffix('%').unwrap_or(text).trim_end();
    if text.is_empty()
        || !text.bytes().any(|b| b.is_ascii_digit())
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    text.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// 日期时间：带偏移的按其时刻，不带偏移的按本地时间；支持 T / 空格分隔与 / 分隔的日期
pub fn datetime(cell: &str) -> Option<f64> {
    let text = cell.trim();
    let bytes = text.as_bytes();
    if bytes.len() < 8
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !matches!(bytes[4], b'-' | b'/')
    {
        return None;
    }
    let owned;
    let text = if bytes[4] == b'/' {
        owned = text.replacen('/', "-", 2);
        owned.as_str()
    } else {
        text
    };
    if let Ok(ts) = text.parse::<Timestamp>() {
        return Some(ts.as_millisecond() as f64);
    }
    let civil = text.parse::<DateTime>().ok().or_else(|| {
        text.parse::<Date>()
            .ok()
            .map(|d| d.to_datetime(jiff::civil::Time::midnight()))
    })?;
    let zoned = civil.to_zoned(local_zone().clone()).ok()?;
    Some(zoned.timestamp().as_millisecond() as f64)
}

/// 列名像时间（time、timestamp、date、ts、*_at、时间、日期）
fn time_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    let last = lower.rsplit(['.', '_', '-', ' ']).next().unwrap_or(&lower);
    ["time", "timestamp", "date", "datetime", "ts", "at"].contains(&last)
        || lower.contains("time")
        || name.contains("时间")
        || name.contains("日期")
}

/// 纯数值时间戳：秒（1e9 ~ 1e10，约 2001–2286 年）或毫秒（1e12 ~ 1e13）
pub fn epoch_ms(value: f64) -> Option<f64> {
    if (1e9..1e10).contains(&value) {
        Some(value * 1000.0)
    } else if (1e12..1e13).contains(&value) {
        Some(value)
    } else {
        None
    }
}

/// 推断一列的类型并给出数值（时间为毫秒）；数值列名像时间且全是合理的时间戳时视为时间
pub fn column(name: &str, raw: &[String]) -> (Kind, Vec<Option<f64>>) {
    let present = raw.iter().filter(|c| !is_missing(c)).count();
    if present == 0 {
        return (Kind::Text, vec![None; raw.len()]);
    }
    let threshold = (present as f64 * RATIO).ceil() as usize;
    let numbers: Vec<Option<f64>> = raw
        .iter()
        .map(|c| if is_missing(c) { None } else { number(c) })
        .collect();
    let parsed = numbers.iter().flatten().count();
    if parsed >= threshold {
        if time_name(name) && numbers.iter().flatten().all(|v| epoch_ms(*v).is_some()) {
            return (
                Kind::Time,
                numbers.into_iter().map(|v| v.and_then(epoch_ms)).collect(),
            );
        }
        return (Kind::Number, numbers);
    }
    let times: Vec<Option<f64>> = raw
        .iter()
        .map(|c| if is_missing(c) { None } else { datetime(c) })
        .collect();
    if times.iter().flatten().count() >= threshold {
        return (Kind::Time, times);
    }
    (Kind::Text, numbers)
}

#[cfg(test)]
#[path = "infer_test.rs"]
mod tests;
