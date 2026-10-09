//! 中国法定节假日与调休：只解析一次 tyme4rs 附带的数据，按日期查表。

use std::collections::HashMap;
use std::sync::OnceLock;

use jiff::civil::{Date, Weekday};
use serde::{Deserialize, Serialize};
use tyme4rs::tyme::holiday::LEGAL_HOLIDAY_DATA;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Holiday {
    /// 节日序号（元旦、春节……），前端翻译名称
    pub index: u8,
    /// true 表示调休上班，false 表示放假
    pub work: bool,
}

struct Table {
    days: HashMap<(i16, i8, i8), Holiday>,
    last_year: i16,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let bytes = LEGAL_HOLIDAY_DATA.as_bytes();
        let mut days = HashMap::new();
        let mut last_year = 0;
        // 每条 13 个字符：年月日 8 位、放假/上班 1 位、节日序号 1 位、相对节日当天的偏移 3 位
        for entry in bytes.chunks(13).filter(|e| e.len() == 13) {
            let text = std::str::from_utf8(entry).unwrap_or_default();
            let (Ok(y), Ok(m), Ok(d)) = (
                text[0..4].parse::<i16>(),
                text[4..6].parse::<i8>(),
                text[6..8].parse::<i8>(),
            ) else {
                continue;
            };
            last_year = last_year.max(y);
            days.insert(
                (y, m, d),
                Holiday {
                    index: entry[9] - b'0',
                    work: entry[8] == b'0',
                },
            );
        }
        Table { days, last_year }
    })
}

pub fn lookup(date: Date) -> Option<Holiday> {
    table()
        .days
        .get(&(date.year(), date.month(), date.day()))
        .copied()
}

/// 有节假日数据的最后一年
pub fn last_year() -> i16 {
    table().last_year
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Calendar {
    /// 周一到周五上班
    #[default]
    Weekdays,
    /// 按中国法定节假日与调休
    China,
}

pub fn is_weekend(date: Date) -> bool {
    matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday)
}

pub fn is_workday(date: Date, calendar: Calendar) -> bool {
    match (calendar, lookup(date)) {
        (Calendar::China, Some(holiday)) => holiday.work,
        _ => !is_weekend(date),
    }
}

#[cfg(test)]
#[path = "holidays_test.rs"]
mod tests;
