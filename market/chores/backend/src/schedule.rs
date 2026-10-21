//! 排期展开与轮换：按计划发生次数推进负责人，历史不因补勾或改名单漂移。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::{format_date, parse_date, weekday};
use crate::model::{Chore, MAX_CHORES, MAX_EPOCHS, MAX_MEMBERS, Member, Mode, RecordMap, Rule};

/// 展开窗口与数量上限：避免长期未打开时生成无限待办
pub const MAX_WINDOW_DAYS: i64 = 120;
pub const MAX_OCCURRENCES: usize = 800;
pub const MAX_PER_CHORE: usize = 300;
/// 每周星期扫描的迭代上限（约 8 年）
const MAX_WEEK_SCAN: i64 = 3000;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArgs {
    pub members: Vec<Member>,
    pub chores: Vec<Chore>,
    #[serde(default)]
    pub records: RecordMap,
    pub today: String,
    pub from_date: String,
    pub to_date: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Occurrence {
    /// 稳定任务编号：`choreId|YYYY-MM-DD`
    pub key: String,
    pub chore_id: String,
    pub chore_name: String,
    pub date: String,
    pub assignee_id: Option<String>,
    pub assignee_name: Option<String>,
    /// pending / done / skipped
    pub status: String,
    /// 已完成但实际日期晚于计划日期（逾期补做，不冒充按时）
    pub late: bool,
    /// 今天之前仍无记录（逾期待做）
    pub missed: bool,
    /// 这一次被临时换过人
    pub overridden: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberStat {
    pub member_id: String,
    pub name: String,
    pub done: u32,
    pub skipped: u32,
    pub pending: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOut {
    pub occurrences: Vec<Occurrence>,
    pub stats: Vec<MemberStat>,
}

/// 规则在 [start, end]（含）内的发生日期
fn enumerate_rule(rule: &Rule, start: i64, end: i64, mut visit: impl FnMut(i64)) {
    if end < start {
        return;
    }
    match rule {
        Rule::Daily => {
            for day in start..=end {
                visit(day);
            }
        }
        Rule::Interval { days } => {
            let step = (*days).max(1) as i64;
            let mut day = start;
            while day <= end {
                visit(day);
                day += step;
            }
        }
        Rule::Weekly { weekdays } => {
            let mut day = start;
            let mut scanned = 0i64;
            while day <= end && scanned < MAX_WEEK_SCAN {
                if weekdays.contains(&(weekday(day) as u8)) {
                    visit(day);
                }
                day += 1;
                scanned += 1;
            }
        }
    }
}

/// [start, end] 内发生次数（用于纪元内的轮换下标推进）
fn count_occurrences(rule: &Rule, start: i64, end: i64) -> usize {
    let mut count = 0usize;
    enumerate_rule(rule, start, end, |_| count += 1);
    count
}

/// 纪元模式下 date 当天按名单轮换到的成员下标
fn member_name(members: &HashMap<&str, &Member>, id: &str) -> Option<String> {
    members.get(id).map(|m| m.name.clone())
}

pub fn plan(args: &PlanArgs) -> PluginResult<PlanOut> {
    if args.members.len() > MAX_MEMBERS {
        return Err(errp("chore.too_many_members", "limit", MAX_MEMBERS));
    }
    if args.chores.len() > MAX_CHORES {
        return Err(errp("chore.too_many_chores", "limit", MAX_CHORES));
    }
    let today = parse_date(&args.today)
        .ok_or_else(|| errp("chore.bad_date", "value", args.today.as_str()))?;
    let from = parse_date(&args.from_date)
        .ok_or_else(|| errp("chore.bad_date", "value", args.from_date.as_str()))?;
    let to = parse_date(&args.to_date)
        .ok_or_else(|| errp("chore.bad_date", "value", args.to_date.as_str()))?;
    if to < from {
        return Err(errp("chore.bad_window", "value", format!("{from}>{to}")));
    }
    if to - from > MAX_WINDOW_DAYS {
        return Err(errp("chore.bad_window", "limit", MAX_WINDOW_DAYS));
    }
    let members: HashMap<&str, &Member> = args.members.iter().map(|m| (m.id.as_str(), m)).collect();

    let mut occurrences = Vec::new();
    for chore in &args.chores {
        if chore.epochs.is_empty() {
            return Err(errp("chore.bad_epochs", "chore", chore.name.as_str()));
        }
        if chore.epochs.len() > MAX_EPOCHS {
            return Err(errp("chore.too_many_epochs", "limit", MAX_EPOCHS));
        }
        let mut previous_from: Option<i64> = None;
        for epoch in &chore.epochs {
            let from_day = parse_date(&epoch.from)
                .ok_or_else(|| errp("chore.bad_date", "value", epoch.from.as_str()))?;
            if let Some(prev) = previous_from
                && from_day <= prev
            {
                return Err(errp("chore.bad_epochs", "chore", chore.name.as_str()));
            }
            previous_from = Some(from_day);
        }
        let paused = chore
            .paused_from
            .as_deref()
            .and_then(parse_date)
            .unwrap_or(i64::MAX);
        let mut per_chore = 0usize;
        for (index, epoch) in chore.epochs.iter().enumerate() {
            let epoch_from = parse_date(&epoch.from).unwrap_or(from);
            let seg_start = from.max(epoch_from);
            let seg_end = match chore.epochs.get(index + 1) {
                Some(next) => (to.min(parse_date(&next.from).unwrap_or(to) - 1)).max(seg_start - 1),
                None => to,
            };
            let seg_end = seg_end.min(paused - 1);
            if seg_end < seg_start || per_chore >= MAX_PER_CHORE {
                continue;
            }
            // 段起点之前该纪元内已发生的次数：轮换下标的起点
            let base = if seg_start > epoch_from {
                count_occurrences(&epoch.rule, epoch_from, seg_start - 1)
            } else {
                0
            };
            let mut ordinal = 0usize;
            enumerate_rule(&epoch.rule, seg_start, seg_end, |day| {
                if per_chore >= MAX_PER_CHORE {
                    return;
                }
                per_chore += 1;
                let date_text = format_date(day);
                let key = format!("{}|{}", chore.id, date_text);
                let record = args.records.get(&key);
                let status = record
                    .and_then(|r| r.status.as_deref())
                    .unwrap_or("pending")
                    .to_owned();
                let actual = record.and_then(|r| r.actual_date.as_deref());
                let late = matches!(status.as_str(), "done")
                    && actual.is_some_and(|text| parse_date(text).is_some_and(|d| d > day));
                let assignee_id = record
                    .and_then(|r| r.assignee.clone())
                    .or_else(|| match &epoch.mode {
                        Mode::Fixed { member_id } => Some(member_id.clone()),
                        Mode::Rotation { member_ids } if !member_ids.is_empty() => {
                            Some(member_ids[(base + ordinal) % member_ids.len()].clone())
                        }
                        Mode::Rotation { .. } => None,
                    });
                let assignee_name = assignee_id
                    .as_deref()
                    .and_then(|id| member_name(&members, id));
                let missed = status == "pending" && day < today;
                occurrences.push(Occurrence {
                    key,
                    chore_id: chore.id.clone(),
                    chore_name: chore.name.clone(),
                    date: date_text,
                    assignee_id,
                    assignee_name,
                    status: if matches!(status.as_str(), "done" | "skipped") {
                        status.clone()
                    } else {
                        "pending".to_owned()
                    },
                    late,
                    missed,
                    overridden: record.is_some_and(|r| r.assignee.is_some()),
                });
                ordinal += 1;
            });
        }
    }
    occurrences
        .sort_by(|a, b| (&a.date, &a.chore_name, &a.key).cmp(&(&b.date, &b.chore_name, &b.key)));
    if occurrences.len() > MAX_OCCURRENCES {
        occurrences.truncate(MAX_OCCURRENCES);
    }

    let mut stats: Vec<MemberStat> = args
        .members
        .iter()
        .map(|m| MemberStat {
            member_id: m.id.clone(),
            name: m.name.clone(),
            done: 0,
            skipped: 0,
            pending: 0,
        })
        .collect();
    let stat_index: HashMap<String, usize> = stats
        .iter()
        .enumerate()
        .map(|(i, s)| (s.member_id.clone(), i))
        .collect();
    for occurrence in &occurrences {
        let Some(record) = args.records.get(&occurrence.key) else {
            if let Some(id) = &occurrence.assignee_id
                && let Some(i) = stat_index.get(id)
            {
                stats[*i].pending += 1;
            }
            continue;
        };
        let target = record
            .executor
            .as_deref()
            .or(occurrence.assignee_id.as_deref())
            .and_then(|id| stat_index.get(id).copied());
        match occurrence.status.as_str() {
            "done" => {
                if let Some(i) = target {
                    stats[i].done += 1;
                }
            }
            "skipped" => {
                if let Some(i) = target {
                    stats[i].skipped += 1;
                }
            }
            _ => {
                if let Some(id) = &occurrence.assignee_id
                    && let Some(i) = stat_index.get(id)
                {
                    stats[*i].pending += 1;
                }
            }
        }
    }

    Ok(PlanOut { occurrences, stats })
}

#[cfg(test)]
#[path = "schedule_test.rs"]
mod tests;
