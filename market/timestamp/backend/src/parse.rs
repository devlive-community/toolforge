use jiff::civil::{Date, DateTime};
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::convert::{ZonedRow, row};
use crate::zone;

#[derive(Deserialize)]
pub struct Args {
    input: String,
    /// 输入不带时区信息时使用的时区
    #[serde(default)]
    timezone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub seconds: i64,
    pub milliseconds: i64,
    pub microseconds: i64,
    pub nanoseconds: String,
    pub iso: String,
    /// 解析出的格式：rfc3339 / zoned / rfc2822 / datetime / date
    pub format: &'static str,
    /// 解析时实际使用的时区（输入自带时区时为其自身时区）
    pub zoned: ZonedRow,
}

/// 依次尝试各种格式，返回带时区的时间与格式名
fn parse(input: &str, timezone: &str) -> PluginResult<(Zoned, &'static str)> {
    if let Ok(zoned) = input.parse::<Zoned>() {
        return Ok((zoned, "zoned"));
    }
    if let Ok(ts) = input.parse::<Timestamp>() {
        return Ok((ts.to_zoned(jiff::tz::TimeZone::UTC), "rfc3339"));
    }
    if let Ok(zoned) = jiff::fmt::rfc2822::parse(input) {
        return Ok((zoned, "rfc2822"));
    }

    let tz = zone::resolve(timezone)?;
    // 兼容 2024/02/29 与 2024.02.29 写法
    let normalized = normalize(input);
    // 没有时间部分时按日期处理（jiff 的 DateTime 也接受纯日期，需先判断）
    if !normalized.contains('T')
        && let Ok(date) = normalized.parse::<Date>()
    {
        let zoned = date
            .to_zoned(tz)
            .map_err(|_| PluginError::new("time.invalid_date"))?;
        return Ok((zoned, "date"));
    }
    if let Ok(dt) = normalized.parse::<DateTime>() {
        let zoned = dt
            .to_zoned(tz)
            .map_err(|_| PluginError::new("time.invalid_date"))?;
        return Ok((zoned, "datetime"));
    }
    Err(PluginError::new("time.unrecognized"))
}

fn normalize(input: &str) -> String {
    let (date, rest) = match input.find([' ', 'T']) {
        Some(index) => input.split_at(index),
        None => (input, ""),
    };
    let date = date.replace(['/', '.'], "-");
    // 补齐 2024-2-9 这类单数字月日
    let parts: Vec<&str> = date.split('-').collect();
    let date = if parts.len() == 3 {
        format!("{}-{:0>2}-{:0>2}", parts[0], parts[1], parts[2])
    } else {
        date
    };
    let rest = rest.trim();
    if rest.is_empty() {
        date
    } else {
        format!("{date}T{}", rest.trim_start_matches('T'))
    }
}

pub fn to_timestamp(args: Args) -> PluginResult<Parsed> {
    let input = args.input.trim();
    if input.is_empty() {
        return Err(PluginError::new("time.empty"));
    }
    let (zoned, format) = parse(input, &args.timezone)?;
    let ts = zoned.timestamp();
    let name = zoned.time_zone().iana_name().unwrap_or("UTC").to_owned();
    Ok(Parsed {
        seconds: ts.as_second(),
        milliseconds: ts.as_millisecond(),
        microseconds: ts.as_microsecond(),
        nanoseconds: ts.as_nanosecond().to_string(),
        iso: ts.to_string(),
        format,
        zoned: row(&name, &zoned),
    })
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
