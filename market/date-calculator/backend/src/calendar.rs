//! 某一天的详细信息（星期、周数、农历、节气、节日、法定节假日、干支、月相、宜忌）与月历。

use jiff::civil::{Date, Weekday};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};
use tyme4rs::tyme::Culture;
use tyme4rs::tyme::culture::PHASE_NAMES;
use tyme4rs::tyme::solar::SolarDay;

use crate::holidays::{self, Holiday};
use crate::scheme::{CustomDay, Rules};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lunar {
    /// 如 农历乙巳年正月初一
    pub text: String,
    pub year: String,
    /// 生肖序号（鼠 = 0）
    pub zodiac: u8,
    pub month: String,
    pub day: String,
    pub leap: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Term {
    /// 节气序号（冬至 = 0）
    pub index: u8,
    /// 进入该节气的第几天（0 表示当天交节）
    pub day: u32,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub date: String,
    /// 星期序号（周一 = 0）
    pub weekday: u8,
    pub iso_year: i16,
    pub iso_week: i8,
    pub day_of_year: i16,
    pub days_in_year: i16,
    pub quarter: u8,
    pub leap_year: bool,
    /// 距今天的天数，过去为负
    pub from_today: i64,
    /// 星座序号（白羊 = 0）
    pub constellation: u8,
    pub lunar: Option<Lunar>,
    pub term: Option<Term>,
    /// 节日名称（中文）
    pub festivals: Vec<String>,
    pub holiday: Option<Holiday>,
    /// 干支：年（按立春）、月（按节气）、日
    pub pillars: Option<[String; 3]>,
    /// 月相序号（新月 = 0，共 8 个）
    pub phase: Option<u8>,
    /// 黄历宜忌（中文数据，传统习俗仅供参考）
    pub recommends: Vec<String>,
    pub avoids: Vec<String>,
}

/// 农历换算支持的范围
fn in_lunar_range(date: Date) -> bool {
    (1900..=2100).contains(&date.year())
}

fn solar(date: Date) -> SolarDay {
    SolarDay::from_ymd(
        date.year() as isize,
        date.month() as usize,
        date.day() as usize,
    )
}

fn lunar(day: &SolarDay) -> Lunar {
    let lunar = day.get_lunar_day();
    let month = lunar.get_lunar_month();
    let year = month.get_lunar_year().get_sixty_cycle();
    Lunar {
        text: lunar.to_string(),
        year: year.get_name(),
        zodiac: year.get_earth_branch().get_index() as u8,
        month: month.get_name(),
        day: lunar.get_name(),
        leap: month.is_leap(),
    }
}

fn festivals(day: &SolarDay) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(f) = day.get_lunar_day().get_festival() {
        out.push(f.get_name());
    }
    if let Some(f) = day.get_festival() {
        out.push(f.get_name());
    }
    out
}

pub fn today() -> Date {
    jiff::Zoned::now().date()
}

pub fn info(date: Date) -> PluginResult<Info> {
    let out_of_range = |_| PluginError::new("date.out_of_range");
    let iso = date.iso_week_date();
    let (lunar_info, term, fest, constellation, almanac) = if in_lunar_range(date) {
        let day = solar(date);
        let term = day.get_term_day();
        let cycle = day.get_sixty_cycle_day();
        let phase_name = day.get_phase().get_name();
        let lunar_day = day.get_lunar_day();
        let almanac = Almanac {
            pillars: Some([
                cycle.get_year().get_name(),
                cycle.get_month().get_name(),
                cycle.get_sixty_cycle().get_name(),
            ]),
            phase: PHASE_NAMES
                .iter()
                .position(|n| *n == phase_name)
                .map(|i| i as u8),
            recommends: lunar_day
                .get_recommends()
                .iter()
                .map(|t| t.get_name())
                .collect(),
            avoids: lunar_day
                .get_avoids()
                .iter()
                .map(|t| t.get_name())
                .collect(),
        };
        (
            Some(lunar(&day)),
            Some(Term {
                index: term.get_solar_term().get_index() as u8,
                day: term.get_day_index() as u32,
            }),
            festivals(&day),
            day.get_constellation().get_index() as u8,
            almanac,
        )
    } else {
        (None, None, Vec::new(), 0, Almanac::default())
    };
    Ok(Info {
        date: date.strftime("%Y-%m-%d").to_string(),
        weekday: date.weekday().to_monday_zero_offset() as u8,
        iso_year: iso.year(),
        iso_week: iso.week(),
        day_of_year: date.day_of_year(),
        days_in_year: date.days_in_year(),
        quarter: ((date.month() - 1) / 3 + 1) as u8,
        leap_year: date.in_leap_year(),
        from_today: today()
            .until((jiff::Unit::Day, date))
            .map_err(out_of_range)?
            .get_days() as i64,
        constellation,
        lunar: lunar_info,
        term,
        festivals: fest,
        holiday: holidays::lookup(date),
        pillars: almanac.pillars,
        phase: almanac.phase,
        recommends: almanac.recommends,
        avoids: almanac.avoids,
    })
}

#[derive(Default)]
struct Almanac {
    pillars: Option<[String; 3]>,
    phase: Option<u8>,
    recommends: Vec<String>,
    avoids: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub date: String,
    pub day: i8,
    pub in_month: bool,
    pub today: bool,
    pub weekend: bool,
    /// 格子里的小字：交节日显示节气，其次节日，初一显示月份，其余显示农历日
    pub label: String,
    /// label 是节气或节日，需要突出显示
    pub special: bool,
    pub holiday: Option<Holiday>,
    /// 所选方案对这一天的覆盖
    pub custom: Option<CustomDay>,
    /// 按所选方案是否上班；未选方案时为 None
    pub workday: Option<bool>,
}

/// 月历：从包含 1 日的那一周开始，共 6 周；scheme 为用来标注的节假日方案
pub fn month(
    year: i16,
    month: i8,
    monday_first: bool,
    scheme: Option<&Rules>,
) -> PluginResult<Vec<Cell>> {
    let first = Date::new(year, month, 1).map_err(|_| PluginError::new("date.out_of_range"))?;
    let start_day = if monday_first {
        Weekday::Monday
    } else {
        Weekday::Sunday
    };
    let back = first.weekday().since(start_day) as i64;
    let mut day = first
        .checked_sub(jiff::Span::new().days(back))
        .map_err(|_| PluginError::new("date.out_of_range"))?;
    let today = today();
    let mut cells = Vec::with_capacity(42);
    for _ in 0..42 {
        let (label, special) = if in_lunar_range(day) {
            let s = solar(day);
            let term = s.get_term_day();
            let lunar_day = s.get_lunar_day();
            if term.get_day_index() == 0 {
                (term.get_solar_term().get_name(), true)
            } else if let Some(f) = festivals(&s).into_iter().next() {
                (f, true)
            } else if lunar_day.get_day() == 1 {
                (lunar_day.get_lunar_month().get_name(), false)
            } else {
                (lunar_day.get_name(), false)
            }
        } else {
            (String::new(), false)
        };
        cells.push(Cell {
            date: day.strftime("%Y-%m-%d").to_string(),
            day: day.day(),
            in_month: day.month() == month,
            today: day == today,
            weekend: holidays::is_weekend(day),
            label,
            special,
            holiday: holidays::lookup(day),
            custom: scheme.and_then(|rules| rules.custom(day).cloned()),
            workday: scheme.map(|rules| rules.is_workday(day)),
        });
        day = match day.tomorrow() {
            Ok(next) => next,
            Err(_) => break,
        };
    }
    Ok(cells)
}

#[cfg(test)]
#[path = "calendar_test.rs"]
mod tests;
