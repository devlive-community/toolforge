//! 解析日期与日期时间输入。

use jiff::civil::{Date, DateTime, Time};
use tf_plugin_api::{PluginError, PluginResult};

/// 输入是否带时间
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moment {
    pub at: DateTime,
    pub has_time: bool,
}

/// 支持 2024-02-29、2024/2/29、2024.2.29、20240229，以及后面跟 HH:MM 或 HH:MM:SS
pub fn moment(input: &str) -> PluginResult<Moment> {
    let invalid = || PluginError::new("date.invalid").with("input", input);
    let text = input.trim().replace('T', " ");
    let mut parts = text.split_whitespace();
    let date_part = parts.next().ok_or_else(invalid)?;
    let time_part = parts.next();
    if parts.next().is_some() {
        return Err(invalid());
    }
    let numbers: Vec<&str> =
        if date_part.len() == 8 && date_part.bytes().all(|b| b.is_ascii_digit()) {
            vec![&date_part[..4], &date_part[4..6], &date_part[6..]]
        } else {
            date_part.split(['-', '/', '.']).collect()
        };
    let [y, m, d] = numbers[..] else {
        return Err(invalid());
    };
    let date = Date::new(
        y.parse().map_err(|_| invalid())?,
        m.parse().map_err(|_| invalid())?,
        d.parse().map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())?;
    let time = match time_part {
        None => Time::midnight(),
        Some(t) => {
            let fields: Vec<&str> = t.split(':').collect();
            if !(2..=3).contains(&fields.len()) {
                return Err(invalid());
            }
            let num = |s: &str| s.parse::<i8>().map_err(|_| invalid());
            Time::new(
                num(fields[0])?,
                num(fields[1])?,
                fields.get(2).map_or(Ok(0), |s| num(s))?,
                0,
            )
            .map_err(|_| invalid())?
        }
    };
    Ok(Moment {
        at: date.to_datetime(time),
        has_time: time_part.is_some(),
    })
}

pub fn date(input: &str) -> PluginResult<Date> {
    moment(input).map(|m| m.at.date())
}

pub fn format(at: DateTime, has_time: bool) -> String {
    if has_time {
        at.strftime("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        at.date().strftime("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
