//! 日期差、日期加减与工作日计算。

use jiff::civil::Date;
use jiff::{Span, Unit};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::holidays::{self, Calendar};
use crate::parse::{self, Moment};

/// 逐日统计工作日的范围上限（约 300 年）
const MAX_SCAN_DAYS: i64 = 110_000;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workdays {
    /// 周一到周五的天数
    pub weekdays: i64,
    /// 按法定节假日与调休计算的工作日
    pub china: i64,
    /// 范围内放假的法定节假日（不含周末）
    pub holidays: i64,
    /// 范围内调休上班的周末
    pub adjusted: i64,
    /// 范围超出了节假日数据覆盖的年份
    pub beyond_data: bool,
    pub data_until: i16,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    /// 结束早于开始
    pub negative: bool,
    pub years: i64,
    pub months: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub total_days: i64,
    pub weeks: i64,
    pub week_days: i64,
    pub total_hours: i64,
    pub total_minutes: i64,
    pub total_seconds: i64,
    pub has_time: bool,
    /// 只对日期计算（开始与结束都不带时间）
    pub workdays: Option<Workdays>,
}

/// 统计 [from, to] 中的工作日（两端都包含）
pub fn count_workdays(from: Date, to: Date) -> PluginResult<Workdays> {
    let (from, to) = if from <= to { (from, to) } else { (to, from) };
    let span = from
        .until(to)
        .map_err(|_| PluginError::new("date.out_of_range"))?;
    if span.get_days() as i64 > MAX_SCAN_DAYS {
        return Err(PluginError::new("date.range_too_large"));
    }
    let mut result = Workdays {
        weekdays: 0,
        china: 0,
        holidays: 0,
        adjusted: 0,
        beyond_data: to.year() > holidays::last_year(),
        data_until: holidays::last_year(),
    };
    let mut day = from;
    loop {
        let weekend = holidays::is_weekend(day);
        if !weekend {
            result.weekdays += 1;
        }
        match holidays::lookup(day) {
            Some(h) if h.work => {
                result.china += 1;
                if weekend {
                    result.adjusted += 1;
                }
            }
            Some(_) if !weekend => result.holidays += 1,
            Some(_) => {}
            None if !weekend => result.china += 1,
            None => {}
        }
        if day == to {
            break;
        }
        day = day
            .tomorrow()
            .map_err(|_| PluginError::new("date.out_of_range"))?;
    }
    Ok(result)
}

pub fn difference(start: Moment, end: Moment, include_end: bool) -> PluginResult<Difference> {
    let has_time = start.has_time || end.has_time;
    let negative = end.at < start.at;
    let (a, b) = if negative {
        (end.at, start.at)
    } else {
        (start.at, end.at)
    };
    // 包含结束日：只对纯日期有意义，多算一天
    let b = if include_end && !has_time {
        b.checked_add(Span::new().days(1))
            .map_err(|_| PluginError::new("date.out_of_range"))?
    } else {
        b
    };
    let out_of_range = |_| PluginError::new("date.out_of_range");
    let calendar = a.until((Unit::Year, b)).map_err(out_of_range)?;
    let total = a.until((Unit::Second, b)).map_err(out_of_range)?;
    let total_seconds = total.get_seconds();
    let total_days = a
        .date()
        .until((Unit::Day, b.date()))
        .map_err(out_of_range)?
        .get_days() as i64;
    let workdays = if has_time {
        None
    } else {
        // 包含结束日时 b 已多加一天，统计到前一天
        let last = b.date().yesterday().map_err(out_of_range)?;
        if last < a.date() {
            None
        } else {
            Some(count_workdays(a.date(), last)?)
        }
    };
    Ok(Difference {
        negative,
        years: calendar.get_years() as i64,
        months: calendar.get_months() as i64,
        days: calendar.get_days() as i64,
        hours: calendar.get_hours() as i64,
        minutes: calendar.get_minutes(),
        seconds: calendar.get_seconds(),
        total_days,
        weeks: total_days / 7,
        week_days: total_days % 7,
        total_hours: total_seconds / 3600,
        total_minutes: total_seconds / 60,
        total_seconds,
        has_time,
        workdays,
    })
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    #[serde(default)]
    pub years: i64,
    #[serde(default)]
    pub months: i64,
    #[serde(default)]
    pub weeks: i64,
    #[serde(default)]
    pub days: i64,
    #[serde(default)]
    pub hours: i64,
    #[serde(default)]
    pub minutes: i64,
    #[serde(default)]
    pub seconds: i64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub result: String,
    pub weekday: u8,
    /// 月末日期被收到当月最后一天（如 1 月 31 日加一个月）
    pub clamped: bool,
}

fn weekday_index(date: Date) -> u8 {
    date.weekday().to_monday_zero_offset() as u8
}

pub fn add(start: Moment, amount: Amount) -> PluginResult<Added> {
    let out_of_range = |_| PluginError::new("date.out_of_range");
    // 年月先加，再加周、日与时间，结果与日常理解一致
    let calendar = Span::new()
        .try_years(amount.years)
        .and_then(|s| s.try_months(amount.months))
        .map_err(out_of_range)?;
    let moved = start.at.checked_add(calendar).map_err(out_of_range)?;
    let clamped = moved.day() != start.at.day() && (amount.years != 0 || amount.months != 0);
    let rest = Span::new()
        .try_weeks(amount.weeks)
        .and_then(|s| s.try_days(amount.days))
        .and_then(|s| s.try_hours(amount.hours))
        .and_then(|s| s.try_minutes(amount.minutes))
        .and_then(|s| s.try_seconds(amount.seconds))
        .map_err(out_of_range)?;
    let result = moved.checked_add(rest).map_err(out_of_range)?;
    let has_time =
        start.has_time || amount.hours != 0 || amount.minutes != 0 || amount.seconds != 0;
    Ok(Added {
        result: parse::format(result, has_time),
        weekday: weekday_index(result.date()),
        clamped,
    })
}

/// 从 start 起（不含当天）往后或往前数 n 个工作日
pub fn add_workdays(start: Date, n: i64, calendar: Calendar) -> PluginResult<Added> {
    if n.unsigned_abs() > MAX_SCAN_DAYS as u64 / 2 {
        return Err(PluginError::new("date.range_too_large"));
    }
    let out_of_range = |_| PluginError::new("date.out_of_range");
    let mut day = start;
    let mut left = n.abs();
    while left > 0 {
        day = if n > 0 {
            day.tomorrow()
        } else {
            day.yesterday()
        }
        .map_err(out_of_range)?;
        if holidays::is_workday(day, calendar) {
            left -= 1;
        }
    }
    Ok(Added {
        result: parse::format(day.to_datetime(jiff::civil::Time::midnight()), false),
        weekday: weekday_index(day),
        clamped: false,
    })
}

#[cfg(test)]
#[path = "calc_test.rs"]
mod tests;
