//! 命令行 / Agent 用的扁平入口：
//! - expiry：给出若干「名称:标签日期」，按界面同样的规则分档（逾期 / 临近 / 正常），临期在前；
//! - convert：按单位登记表换算数量（g/kg、ml/l 同维换算，计数单位不互相换算）。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::{format_date, parse_date};
use crate::review::date_state;
use crate::units::{self, UnitError};

/// 每次最多检查的条目数
const MAX_ITEMS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpiryArgs {
    pub items: Vec<String>,
    /// 今天（YYYY-MM-DD），缺省取本机日期
    #[serde(default)]
    pub today: Option<String>,
    #[serde(default = "three")]
    pub soon_days: u32,
}

fn three() -> u32 {
    3
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExpiryRow {
    pub name: String,
    pub date: String,
    /// overdue | soon | ok
    pub state: String,
    /// 距标签日期的天数（负数为已过期）
    pub days: i64,
}

fn local_today() -> String {
    jiff::Zoned::now().date().to_string()
}

pub fn expiry(args: ExpiryArgs) -> PluginResult<Vec<ExpiryRow>> {
    if args.items.is_empty() || args.items.len() > MAX_ITEMS {
        return Err(PluginError::new("fridge.bad_items").with("limit", MAX_ITEMS));
    }
    if args.soon_days > 365 {
        return Err(PluginError::new("fridge.bad_amount"));
    }
    let today_text = args.today.clone().unwrap_or_else(local_today);
    let today = parse_date(&today_text)
        .ok_or_else(|| PluginError::new("fridge.bad_date").with("value", today_text.as_str()))?;
    let mut rows = Vec::with_capacity(args.items.len());
    for line in &args.items {
        // 名称里可能有冒号，取最后一个冒号后的日期
        let (name, date) = line
            .rsplit_once(':')
            .ok_or_else(|| PluginError::new("fridge.bad_item").with("value", line.as_str()))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(PluginError::new("fridge.name_required"));
        }
        let day = parse_date(date.trim())
            .ok_or_else(|| PluginError::new("fridge.bad_date").with("value", date.trim()))?;
        rows.push(ExpiryRow {
            name: name.to_string(),
            date: format_date(day),
            state: date_state(day, today, args.soon_days),
            days: day - today,
        });
    }
    rows.sort_by(|a, b| a.days.cmp(&b.days).then_with(|| a.name.cmp(&b.name)));
    Ok(rows)
}

#[derive(Debug, Deserialize)]
pub struct ConvertArgs {
    pub amount: f64,
    pub from: String,
    pub to: String,
}

pub fn convert(args: ConvertArgs) -> PluginResult<serde_json::Value> {
    if !args.amount.is_finite() || args.amount < 0.0 || args.amount > 1e9 {
        return Err(PluginError::new("fridge.bad_amount"));
    }
    let from = args.from.trim().to_lowercase();
    let to = args.to.trim().to_lowercase();
    let value = units::convert(args.amount, &from, &to).map_err(|e| match e {
        UnitError::Unknown => PluginError::new("fridge.bad_unit").with(
            "value",
            if units::known(&from) {
                to.clone()
            } else {
                from.clone()
            },
        ),
        UnitError::Mismatch => {
            PluginError::new("fridge.unit_mismatch").with("value", format!("{from} → {to}"))
        }
    })?;
    // 去掉浮点尾差，保留至多 6 位小数
    let value = (value * 1e6).round() / 1e6;
    Ok(serde_json::json!({ "value": value, "unit": to, "text": format!("{value} {to}") }))
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
