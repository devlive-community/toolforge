//! 生肖与星座查询、农历纪念日倒计时。
//!
//! 生肖有两种口径：按春节（农历年）与按立春（干支纪年）。两者在春节与立春之间的日子会不同。
//! 星座按公历日期划分，日期边界与 tyme4rs 一致。

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};
use tyme4rs::tyme::Culture;
use tyme4rs::tyme::lunar::LunarDay;
use tyme4rs::tyme::solar::SolarDay;

use crate::calendar;

/// 每个月的最后一天仍属上一个星座的日期（与 tyme4rs SolarDay::get_constellation 相同）
const CUTOFF: [i8; 12] = [19, 18, 20, 19, 20, 21, 22, 22, 22, 23, 22, 21];

fn out_of_range() -> PluginError {
    PluginError::new("date.out_of_range")
}

fn solar_of(date: Date) -> SolarDay {
    SolarDay::from_ymd(
        date.year() as isize,
        date.month() as usize,
        date.day() as usize,
    )
}

fn date_of(day: SolarDay) -> PluginResult<Date> {
    Date::new(
        day.get_year() as i16,
        day.get_month() as i8,
        day.get_day() as i8,
    )
    .map_err(|_| out_of_range())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LunarInput {
    pub year: i16,
    pub month: u8,
    pub day: u8,
    #[serde(default)]
    pub leap: bool,
}

/// 农历日期；闰月用负数月份表示（tyme4rs 约定）。不存在的日期返回错误
fn lunar_day(input: &LunarInput) -> PluginResult<LunarDay> {
    if !(1900..=2100).contains(&input.year) || !(1..=12).contains(&input.month) {
        return Err(out_of_range());
    }
    let invalid = || {
        PluginError::new("date.lunar_invalid")
            .with("month", input.month)
            .with("day", input.day)
            .with("leap", input.leap)
    };
    // tyme4rs 遇到不存在的闰月会 panic，先确认当年确实闰这个月
    let leap_month =
        tyme4rs::tyme::lunar::LunarYear::from_year(input.year as isize).get_leap_month();
    if input.leap && leap_month != input.month as usize {
        return Err(invalid());
    }
    let month = if input.leap {
        -(input.month as isize)
    } else {
        input.month as isize
    };
    LunarDay::new(input.year as isize, month, input.day as usize).map_err(|_| invalid())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "calendar", rename_all = "camelCase")]
pub enum BirthArgs {
    Solar { date: String },
    Lunar(LunarInput),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub index: u8,
    /// [月, 日]
    pub start: [i8; 2],
    pub end: [i8; 2],
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Birth {
    pub date: String,
    pub lunar: String,
    /// 按春节（农历年）的生肖序号，鼠 = 0
    pub zodiac_lunar_year: u8,
    pub lunar_year_ganzhi: String,
    /// 按立春（干支纪年）的生肖序号
    pub zodiac_lichun: u8,
    pub lichun_year_ganzhi: String,
    /// 本年立春日期
    pub lichun: String,
    /// 本年春节（农历正月初一）日期
    pub spring_festival: String,
    /// 星座序号，白羊 = 0
    pub constellation: u8,
    pub constellations: Vec<Range>,
}

/// 十二星座的日期范围（白羊起）
pub fn ranges() -> Vec<Range> {
    (0..12u8)
        .map(|index| {
            // 白羊从 3 月开始：星座 i 起始于 (i + 2) 月份的 CUTOFF + 1
            let start_month = (index as usize + 2) % 12;
            let end_month = (start_month + 1) % 12;
            Range {
                index,
                start: [start_month as i8 + 1, CUTOFF[start_month] + 1],
                end: [end_month as i8 + 1, CUTOFF[end_month]],
            }
        })
        .collect()
}

pub fn birth(args: BirthArgs) -> PluginResult<Birth> {
    let solar = match args {
        BirthArgs::Solar { date } => {
            let date = crate::parse::date(&date)?;
            if !(1900..=2100).contains(&date.year()) {
                return Err(out_of_range());
            }
            solar_of(date)
        }
        BirthArgs::Lunar(input) => lunar_day(&input)?.get_solar_day(),
    };
    let date = date_of(solar)?;
    let lunar = solar.get_lunar_day();
    let lunar_year = lunar.get_lunar_month().get_lunar_year();
    let lunar_cycle = lunar_year.get_sixty_cycle();
    let lichun_cycle = solar.get_sixty_cycle_day().get_year();
    let lichun =
        tyme4rs::tyme::solar::SolarTerm::from_index(date.year() as isize, 3).get_solar_day();
    let spring = LunarDay::from_ymd(date.year() as isize, 1, 1).get_solar_day();
    Ok(Birth {
        date: date.strftime("%Y-%m-%d").to_string(),
        lunar: lunar.to_string(),
        zodiac_lunar_year: lunar_cycle.get_earth_branch().get_index() as u8,
        lunar_year_ganzhi: lunar_cycle.get_name(),
        zodiac_lichun: lichun_cycle.get_earth_branch().get_index() as u8,
        lichun_year_ganzhi: lichun_cycle.get_name(),
        lichun: date_of(lichun)?.strftime("%Y-%m-%d").to_string(),
        spring_festival: date_of(spring)?.strftime("%Y-%m-%d").to_string(),
        constellation: solar.get_constellation().get_index() as u8,
        constellations: ranges(),
    })
}

#[derive(Debug, Deserialize)]
pub struct AnniversaryArgs {
    pub items: Vec<LunarInput>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Upcoming {
    /// 下一次（含今天）对应的公历日期
    pub date: String,
    pub lunar: String,
    pub days: i64,
    /// 当年没有该闰月或该日（如腊月三十）时改用的说明：noLeap / lastDay
    pub adjusted: Option<&'static str>,
}

/// 某个农历年中纪念日对应的日子：没有闰月时用平月，没有该日（30 日）时用当月最后一天
fn occurrence(year: isize, item: &LunarInput) -> Option<(LunarDay, Option<&'static str>)> {
    let mut adjusted = None;
    let mut month = item.month as isize;
    if item.leap {
        let leap = tyme4rs::tyme::lunar::LunarYear::from_year(year).get_leap_month();
        if leap as isize == month {
            month = -month;
        } else {
            adjusted = Some("noLeap");
        }
    }
    let days = tyme4rs::tyme::lunar::LunarMonth::from_ym(year, month).get_day_count();
    let day = if (item.day as usize) > days {
        adjusted = Some("lastDay");
        days
    } else {
        item.day as usize
    };
    LunarDay::new(year, month, day).ok().map(|d| (d, adjusted))
}

pub fn anniversaries(args: AnniversaryArgs) -> PluginResult<Vec<Upcoming>> {
    if args.items.len() > 200 {
        return Err(PluginError::new("date.too_many").with("max", 200));
    }
    let today = calendar::today();
    let current = solar_of(today)
        .get_lunar_day()
        .get_lunar_month()
        .get_lunar_year()
        .get_year();
    args.items
        .iter()
        .map(|item| {
            if !(1..=12).contains(&item.month) || !(1..=30).contains(&item.day) {
                return Err(PluginError::new("date.lunar_invalid")
                    .with("month", item.month)
                    .with("day", item.day));
            }
            // 今年的已经过去就看明年
            for year in [current, current + 1] {
                if !(1900..=2100).contains(&year) {
                    continue;
                }
                let Some((day, adjusted)) = occurrence(year, item) else {
                    continue;
                };
                let date = date_of(day.get_solar_day())?;
                if date >= today {
                    let days = today
                        .until((jiff::Unit::Day, date))
                        .map_err(|_| out_of_range())?
                        .get_days() as i64;
                    return Ok(Upcoming {
                        date: date.strftime("%Y-%m-%d").to_string(),
                        lunar: day.to_string(),
                        days,
                        adjusted,
                    });
                }
            }
            Err(out_of_range())
        })
        .collect()
}

#[cfg(test)]
#[path = "zodiac_test.rs"]
mod tests;
