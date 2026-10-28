//! 命令行 / Agent 用的扁平入口：给出轮换名单与重复规则，直接排出一段日期内的值日表。
//! 复用界面同一套排期逻辑（schedule::plan），结果与界面里的轮换完全一致。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::{format_date, parse_date};
use crate::model::{Chore, Epoch, MAX_MEMBERS, Member, Mode, Rule};
use crate::schedule::{MAX_WINDOW_DAYS, PlanArgs, plan};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotaArgs {
    /// 轮换名单（按顺序轮流）
    pub members: Vec<String>,
    /// 第一次（轮换起点）的日期
    pub start: String,
    #[serde(default = "daily")]
    pub rule: String,
    /// 每周规则的星期：0 = 周日 … 6 = 周六
    #[serde(default)]
    pub weekdays: Vec<u8>,
    /// 间隔规则的天数
    #[serde(default = "one")]
    pub every: u32,
    /// 列出的起始日期，缺省为 start
    #[serde(default)]
    pub from: Option<String>,
    /// 列出多少天
    #[serde(default = "two_weeks")]
    pub days: i64,
    #[serde(default)]
    pub name: String,
}

fn daily() -> String {
    "daily".into()
}
fn one() -> u32 {
    1
}
fn two_weeks() -> i64 {
    14
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RotaRow {
    pub date: String,
    pub member: String,
}

fn bad(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

pub fn rota(args: RotaArgs) -> PluginResult<Vec<RotaRow>> {
    let members: Vec<String> = args
        .members
        .iter()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();
    if members.is_empty() {
        return Err(PluginError::new("chore.rotation_required"));
    }
    if members.len() > MAX_MEMBERS {
        return Err(bad("chore.too_many_members", "limit", MAX_MEMBERS));
    }
    let start = parse_date(&args.start)
        .ok_or_else(|| bad("chore.bad_date", "value", args.start.as_str()))?;
    let from_text = args.from.clone().unwrap_or_else(|| args.start.clone());
    let from =
        parse_date(&from_text).ok_or_else(|| bad("chore.bad_date", "value", from_text.as_str()))?;
    if args.days < 1 || args.days > MAX_WINDOW_DAYS + 1 {
        return Err(bad("chore.bad_window", "limit", MAX_WINDOW_DAYS + 1));
    }
    let rule = match args.rule.as_str() {
        "daily" => Rule::Daily,
        "weekly" => {
            if args.weekdays.is_empty() || args.weekdays.iter().any(|d| *d > 6) {
                return Err(PluginError::new("chore.weekday_required"));
            }
            Rule::Weekly {
                weekdays: args.weekdays.clone(),
            }
        }
        "interval" => {
            if args.every == 0 || args.every > 365 {
                return Err(bad("chore.bad_window", "limit", 365));
            }
            Rule::Interval { days: args.every }
        }
        other => return Err(bad("chore.bad_rule", "value", other)),
    };
    let ids: Vec<String> = (0..members.len()).map(|i| format!("m{i}")).collect();
    let plan_args = PlanArgs {
        members: members
            .iter()
            .zip(&ids)
            .map(|(name, id)| Member {
                id: id.clone(),
                name: name.clone(),
            })
            .collect(),
        chores: vec![Chore {
            id: "rota".into(),
            name: args.name.clone(),
            epochs: vec![Epoch {
                from: format_date(start),
                rule,
                mode: Mode::Rotation { member_ids: ids },
            }],
            paused_from: None,
        }],
        records: Default::default(),
        // 只关心负责人，不判断逾期：today 取窗口起点
        today: format_date(from),
        from_date: format_date(from),
        to_date: format_date(from + args.days - 1),
    };
    Ok(plan(&plan_args)?
        .occurrences
        .into_iter()
        .map(|o| RotaRow {
            date: o.date,
            member: o.assignee_name.unwrap_or_default(),
        })
        .collect())
}

#[cfg(test)]
#[path = "rota_test.rs"]
mod tests;
