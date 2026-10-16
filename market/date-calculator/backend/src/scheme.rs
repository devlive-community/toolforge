//! 自定义节假日方案：在「中国法定节假日」或「周一至周五」的基础上，指定哪些星期是周末，
//! 再逐日覆盖为上班或休息（公司假期、其他国家或地区的节假日、临时调休）。
//!
//! 方案由界面保存在插件状态中，每次计算时随参数传入；导入导出使用 JSON。

use std::collections::{BTreeMap, HashMap};

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{PluginError, PluginResult};

use crate::holidays::{self, Calendar};

/// 一个方案最多的自定义日期
pub const MAX_DAYS: usize = 5000;
/// 导出文件的格式标记
pub const FORMAT: &str = "toolforge-holiday-scheme";

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    /// 中国法定节假日与调休
    #[default]
    China,
    /// 只按周末休息
    Weekdays,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomDay {
    pub date: String,
    /// true 为上班，false 为休息
    pub work: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn default_weekend() -> Vec<u8> {
    vec![6, 7]
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Scheme {
    pub name: String,
    #[serde(default)]
    pub base: Base,
    /// 周末（ISO：1 = 周一 … 7 = 周日）
    #[serde(default = "default_weekend")]
    pub weekend: Vec<u8>,
    #[serde(default)]
    pub days: Vec<CustomDay>,
}

/// 工作日规则：内置日历或自定义方案
#[derive(Debug, Clone)]
pub struct Rules {
    base: Base,
    weekend: [bool; 7],
    overrides: HashMap<Date, CustomDay>,
}

/// 计算参数中的日历："china" / "weekdays" 或一个完整方案
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CalendarArg {
    Named(Calendar),
    Custom(Scheme),
}

impl Default for CalendarArg {
    fn default() -> Self {
        Self::Named(Calendar::default())
    }
}

impl CalendarArg {
    pub fn rules(&self) -> PluginResult<Rules> {
        match self {
            Self::Named(Calendar::China) => Ok(Rules::builtin(Base::China)),
            Self::Named(Calendar::Weekdays) => Ok(Rules::builtin(Base::Weekdays)),
            Self::Custom(scheme) => Rules::from_scheme(scheme),
        }
    }
}

fn parse_date(text: &str) -> PluginResult<Date> {
    text.trim()
        .parse::<Date>()
        .map_err(|_| PluginError::new("scheme.invalid_date").with("date", text))
}

/// 校验并规范化：名称必填；周末去重排序；同一天重复时以最后一条为准；日期排序
pub fn normalize(scheme: Scheme) -> PluginResult<Scheme> {
    let name = scheme.name.trim().to_owned();
    if name.is_empty() {
        return Err(PluginError::new("scheme.name_required"));
    }
    if scheme.days.len() > MAX_DAYS {
        return Err(PluginError::new("scheme.too_many").with("max", MAX_DAYS));
    }
    let mut weekend: Vec<u8> = scheme.weekend.clone();
    if let Some(bad) = weekend.iter().find(|d| !(1..=7).contains(*d)) {
        return Err(PluginError::new("scheme.invalid_weekday").with("day", *bad));
    }
    weekend.sort_unstable();
    weekend.dedup();
    let mut days: BTreeMap<Date, CustomDay> = BTreeMap::new();
    for day in scheme.days {
        let date = parse_date(&day.date)?;
        let name = day
            .name
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty());
        days.insert(
            date,
            CustomDay {
                date: date.strftime("%Y-%m-%d").to_string(),
                work: day.work,
                name,
            },
        );
    }
    Ok(Scheme {
        name,
        base: scheme.base,
        weekend,
        days: days.into_values().collect(),
    })
}

/// 导入：接受导出的文件，也接受只有方案字段的 JSON
pub fn import(text: &str) -> PluginResult<Scheme> {
    let scheme: Scheme = serde_json::from_str(text)
        .map_err(|e| PluginError::new("scheme.invalid_json").with("detail", e.to_string()))?;
    normalize(scheme)
}

pub fn export(scheme: Scheme) -> PluginResult<String> {
    let scheme = normalize(scheme)?;
    serde_json::to_string_pretty(&json!({
        "format": FORMAT,
        "version": 1,
        "name": scheme.name,
        "base": scheme.base,
        "weekend": scheme.weekend,
        "days": scheme.days,
    }))
    .map_err(|e| PluginError::new("scheme.invalid_json").with("detail", e.to_string()))
}

/// 一天按规则得出的结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// 方案逐日覆盖
    Custom,
    /// 法定节假日或调休
    Legal,
    Weekend,
    Weekday,
}

impl Rules {
    pub fn builtin(base: Base) -> Self {
        Self {
            base,
            weekend: weekend_mask(&default_weekend()),
            overrides: HashMap::new(),
        }
    }

    pub fn from_scheme(scheme: &Scheme) -> PluginResult<Self> {
        let scheme = normalize(scheme.clone())?;
        let mut overrides = HashMap::with_capacity(scheme.days.len());
        for day in scheme.days {
            overrides.insert(parse_date(&day.date)?, day);
        }
        Ok(Self {
            base: scheme.base,
            weekend: weekend_mask(&scheme.weekend),
            overrides,
        })
    }

    pub fn is_weekend(&self, date: Date) -> bool {
        self.weekend[date.weekday().to_monday_zero_offset() as usize]
    }

    pub fn custom(&self, date: Date) -> Option<&CustomDay> {
        self.overrides.get(&date)
    }

    /// 是否上班，以及决定结果的来源
    pub fn day(&self, date: Date) -> (bool, Source) {
        if let Some(day) = self.overrides.get(&date) {
            return (day.work, Source::Custom);
        }
        if self.base == Base::China
            && let Some(holiday) = holidays::lookup(date)
        {
            return (holiday.work, Source::Legal);
        }
        if self.is_weekend(date) {
            (false, Source::Weekend)
        } else {
            (true, Source::Weekday)
        }
    }

    pub fn is_workday(&self, date: Date) -> bool {
        self.day(date).0
    }

    /// 依赖法定节假日数据且日期超出数据覆盖范围
    pub fn beyond_data(&self, date: Date) -> bool {
        self.base == Base::China && date.year() > holidays::last_year()
    }
}

fn weekend_mask(days: &[u8]) -> [bool; 7] {
    let mut mask = [false; 7];
    for day in days {
        if (1..=7).contains(day) {
            mask[(*day - 1) as usize] = true;
        }
    }
    mask
}

#[cfg(test)]
#[path = "scheme_test.rs"]
mod tests;
