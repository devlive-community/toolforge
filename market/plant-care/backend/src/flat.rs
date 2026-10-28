//! 命令行 / Agent 用的扁平入口：给出每项养护的间隔与最近一次实际日期，算出下次日期与剩余天数。
//!
//! 写法：`名称:间隔天数[:最近一次日期]`。下次日期 = 最近一次实际日期 + 间隔（与界面相同），
//! 没有写最近一次时视为今天到期。间隔范围与日期校验复用界面同一套规则。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::date::format_date;
use crate::model::{day, error, interval, text};

const MAX_ITEMS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DueArgs {
    pub items: Vec<String>,
    #[serde(default)]
    pub today: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DueRow {
    pub name: String,
    pub last: String,
    pub next: String,
    /// 距下次日期的天数（负数为已逾期）
    pub days: i64,
    /// overdue | today | upcoming
    pub state: String,
}

pub fn due(args: DueArgs, local_today: &str) -> PluginResult<Vec<DueRow>> {
    if args.items.is_empty() || args.items.len() > MAX_ITEMS {
        return Err(error("limits"));
    }
    let today_text = args.today.unwrap_or_else(|| local_today.to_string());
    let today = day(&today_text)?;
    let mut rows = Vec::with_capacity(args.items.len());
    for line in &args.items {
        let parts: Vec<&str> = line.split(':').map(str::trim).collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(error("format"));
        }
        let name = text(parts[0], 80, true)?;
        let every = interval(parts[1])?;
        let (last, next) = match parts.get(2).filter(|s| !s.is_empty()) {
            Some(last) => {
                let last_day = day(last)?;
                if last_day > today {
                    return Err(error("future"));
                }
                (format_date(last_day), last_day + i64::from(every))
            }
            None => (String::new(), today),
        };
        let days = next - today;
        rows.push(DueRow {
            name,
            last,
            next: format_date(next),
            days,
            state: match days {
                d if d < 0 => "overdue",
                0 => "today",
                _ => "upcoming",
            }
            .to_string(),
        });
    }
    rows.sort_by(|a, b| a.days.cmp(&b.days).then_with(|| a.name.cmp(&b.name)));
    Ok(rows)
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
