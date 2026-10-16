use jiff::civil::{Date, DateTime};
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::convert::{ZonedRow, row};
use crate::{natural, zone};

#[derive(Deserialize)]
pub struct Args {
    input: String,
    /// 输入不带时区信息时使用的时区
    #[serde(default)]
    timezone: String,
    /// 自然语言有多种解释时选用第几个
    #[serde(default)]
    choice: usize,
}

/// 自然语言解析的一种解释
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateView {
    /// 所选时区下的日期时间
    pub datetime: String,
    /// 1 = 周一 … 7 = 周日（与 from_timestamp 一致）
    pub weekday: i8,
    pub notes: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub seconds: i64,
    pub milliseconds: i64,
    pub microseconds: i64,
    pub nanoseconds: String,
    pub iso: String,
    /// 解析出的格式：rfc3339 / zoned / rfc2822 / datetime / date / natural
    pub format: &'static str,
    /// 自然语言的全部解释（其他格式为空）
    pub candidates: Vec<CandidateView>,
    pub choice: usize,
    /// 解析时实际使用的时区（输入自带时区时为其自身时区）
    pub zoned: ZonedRow,
}

type Natural = Vec<natural::Candidate>;

/// 依次尝试各种格式，都不匹配时按自然语言解析；now 在测试中固定，为空时取当前时间
fn parse_at(
    input: &str,
    timezone: &str,
    now: Option<&Zoned>,
) -> PluginResult<(Zoned, &'static str, Natural)> {
    let plain = |(zoned, format)| (zoned, format, Vec::new());
    parse_formats(input, timezone).map(plain).or_else(|err| {
        if err.code != "time.unrecognized" {
            return Err(err);
        }
        let tz = zone::resolve(timezone)?;
        let now = now
            .cloned()
            .unwrap_or_else(|| Zoned::now().with_time_zone(tz));
        match natural::parse(input, &now) {
            Some(candidates) => Ok((candidates[0].at.clone(), "natural", candidates)),
            None => Err(err),
        }
    })
}

/// 精确格式，返回带时区的时间与格式名
fn parse_formats(input: &str, timezone: &str) -> PluginResult<(Zoned, &'static str)> {
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
    to_timestamp_at(args, None)
}

fn to_timestamp_at(args: Args, now: Option<&Zoned>) -> PluginResult<Parsed> {
    let input = args.input.trim();
    if input.is_empty() {
        return Err(PluginError::new("time.empty"));
    }
    let (first, format, candidates) = parse_at(input, &args.timezone, now)?;
    let choice = args.choice.min(candidates.len().saturating_sub(1));
    let zoned = candidates.get(choice).map_or(first, |c| c.at.clone());
    let ts = zoned.timestamp();
    let name = zoned.time_zone().iana_name().unwrap_or("UTC").to_owned();
    Ok(Parsed {
        seconds: ts.as_second(),
        milliseconds: ts.as_millisecond(),
        microseconds: ts.as_microsecond(),
        nanoseconds: ts.as_nanosecond().to_string(),
        iso: ts.to_string(),
        format,
        candidates: candidates
            .iter()
            .map(|c| CandidateView {
                datetime: c.at.strftime("%Y-%m-%d %H:%M:%S").to_string(),
                weekday: c.at.weekday().to_monday_one_offset(),
                notes: c.notes.clone(),
            })
            .collect(),
        choice,
        zoned: row(&name, &zoned),
    })
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
