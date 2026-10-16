use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::zone;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    #[default]
    Auto,
    S,
    Ms,
    Us,
    Ns,
}

#[derive(Deserialize)]
pub struct Args {
    value: String,
    #[serde(default)]
    unit: Unit,
    #[serde(default)]
    timezones: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonedRow {
    pub timezone: String,
    pub datetime: String,
    pub offset: String,
    pub abbreviation: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Converted {
    pub unit: Unit,
    pub seconds: i64,
    pub milliseconds: i64,
    pub iso: String,
    pub rfc2822: String,
    /// 1 = 周一 … 7 = 周日
    pub weekday: i8,
    pub day_of_year: i16,
    pub iso_week: i8,
    /// 相对当前时间的秒数（正数表示未来）
    pub relative_seconds: i64,
    pub rows: Vec<ZonedRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Now {
    pub seconds: i64,
    pub milliseconds: i64,
    pub iso: String,
    pub local: ZonedRow,
}

/// 按位数识别单位：≤11 位秒、≤14 位毫秒、≤17 位微秒，否则纳秒
pub fn detect(digits: usize) -> Unit {
    match digits {
        0..=11 => Unit::S,
        12..=14 => Unit::Ms,
        15..=17 => Unit::Us,
        _ => Unit::Ns,
    }
}

pub fn row(name: &str, zoned: &Zoned) -> ZonedRow {
    ZonedRow {
        timezone: name.to_owned(),
        datetime: zoned.strftime("%Y-%m-%d %H:%M:%S%.3f").to_string(),
        offset: zoned.strftime("%:z").to_string(),
        abbreviation: zoned.strftime("%Z").to_string(),
    }
}

fn to_timestamp(value: i128, unit: Unit) -> PluginResult<Timestamp> {
    let result = match unit {
        Unit::S | Unit::Auto => i64::try_from(value)
            .ok()
            .and_then(|v| Timestamp::from_second(v).ok()),
        Unit::Ms => i64::try_from(value)
            .ok()
            .and_then(|v| Timestamp::from_millisecond(v).ok()),
        Unit::Us => i64::try_from(value)
            .ok()
            .and_then(|v| Timestamp::from_microsecond(v).ok()),
        Unit::Ns => Timestamp::from_nanosecond(value).ok(),
    };
    result.ok_or_else(|| PluginError::new("time.out_of_range"))
}

pub fn from_timestamp(args: Args) -> PluginResult<Converted> {
    let raw = args.value.trim().replace(['_', ','], "");
    if raw.is_empty() {
        return Err(PluginError::new("time.empty"));
    }
    let value: i128 = raw
        .parse()
        .map_err(|_| PluginError::new("time.not_a_number"))?;
    let unit = match args.unit {
        Unit::Auto => detect(raw.trim_start_matches('-').len()),
        unit => unit,
    };
    let ts = to_timestamp(value, unit)?;

    let utc = ts.to_zoned(jiff::tz::TimeZone::UTC);
    let mut rows = Vec::new();
    for name in std::iter::once(zone::LOCAL.to_owned())
        .chain(std::iter::once("UTC".to_owned()))
        .chain(
            args.timezones
                .into_iter()
                .filter(|z| z != zone::LOCAL && z != "UTC"),
        )
    {
        let tz = zone::resolve(&name)?;
        rows.push(row(&zone::display_name(&name, &tz), &ts.to_zoned(tz)));
    }

    let date = utc.date();
    Ok(Converted {
        unit,
        seconds: ts.as_second(),
        milliseconds: ts.as_millisecond(),
        iso: ts.to_string(),
        rfc2822: jiff::fmt::rfc2822::to_string(&utc).unwrap_or_default(),
        weekday: date.weekday().to_monday_one_offset(),
        day_of_year: date.day_of_year(),
        iso_week: date.iso_week_date().week(),
        relative_seconds: ts.as_second() - Timestamp::now().as_second(),
        rows,
    })
}

pub fn now() -> Now {
    let ts = Timestamp::now();
    let tz = jiff::tz::TimeZone::system();
    let name = zone::display_name(zone::LOCAL, &tz);
    Now {
        seconds: ts.as_second(),
        milliseconds: ts.as_millisecond(),
        iso: ts.to_string(),
        local: row(&name, &ts.to_zoned(tz)),
    }
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
