//! 命令行 / Agent 用的扁平入口：给出起针数与加减针规则，列出哪几行／圈要加减针以及之后的针数。
//!
//! 规则写法：`名称:起始行:间隔:次数:增减针`（如 `袖下加针:7:6:8:2` 表示从第 7 行起每 6 行加 2 针，共 8 次）。
//! 到期判断复用部件提醒同一套规则（Reminder::occurrence），所有规则都按执行计算。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::model::{MAX_ROWS, MAX_STITCHES, Reminder, error, label};

/// 每次最多的规则数与列出的变更行数
const MAX_RULES: usize = 50;
const MAX_LINES: usize = 5000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapingArgs {
    pub stitches: u32,
    pub rules: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// 行／圈号
    pub line: u32,
    /// 这一行到期的规则（名称与第几次）
    pub changes: String,
    /// 这一行的针数变化
    pub delta: i64,
    /// 完成这一行后的针数
    pub stitches: u32,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShapingOut {
    pub lines: Vec<Line>,
    pub final_stitches: u32,
    /// 最后一次变更所在的行／圈
    pub last_line: u32,
}

fn number(text: &str) -> PluginResult<u32> {
    text.trim().parse().map_err(|_| error("number"))
}

fn rule(text: &str, index: usize) -> PluginResult<Reminder> {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() != 5 {
        return Err(error("reminder"));
    }
    let delta: i32 = parts[4].trim().parse().map_err(|_| error("reminder"))?;
    let reminder = Reminder {
        id: format!("r{index}"),
        name: label(parts[0])?,
        start: number(parts[1])?,
        interval: number(parts[2])?,
        repeats: number(parts[3])?,
        delta,
    };
    let last = u64::from(reminder.start)
        + u64::from(reminder.interval) * u64::from(reminder.repeats.saturating_sub(1));
    if reminder.start == 0
        || reminder.interval == 0
        || !(1..=10_000).contains(&reminder.repeats)
        || delta == 0
        || last > u64::from(MAX_ROWS)
    {
        return Err(error("reminder"));
    }
    Ok(reminder)
}

pub fn shaping(args: ShapingArgs) -> PluginResult<ShapingOut> {
    if args.rules.is_empty() || args.rules.len() > MAX_RULES || args.stitches > MAX_STITCHES {
        return Err(error("limits"));
    }
    let rules = args
        .rules
        .iter()
        .enumerate()
        .map(|(i, r)| rule(r, i))
        .collect::<PluginResult<Vec<_>>>()?;
    // 只逐个检查规则会到期的行，不遍历全部 10 万行
    let mut due: Vec<u32> = rules
        .iter()
        .flat_map(|r| (0..r.repeats).map(move |k| r.start + r.interval * k))
        .collect();
    due.sort_unstable();
    due.dedup();
    if due.len() > MAX_LINES {
        return Err(error("limits"));
    }
    let mut stitches = i64::from(args.stitches);
    let mut lines = Vec::with_capacity(due.len());
    for line in due {
        let mut delta = 0i64;
        let mut names = Vec::new();
        for r in &rules {
            if let Some(n) = r.occurrence(line) {
                delta += i64::from(r.delta);
                names.push(format!("{} #{n}", r.name));
            }
        }
        stitches += delta;
        if !(0..=i64::from(MAX_STITCHES)).contains(&stitches) {
            return Err(error("stitches"));
        }
        lines.push(Line {
            line,
            changes: names.join(", "),
            delta,
            stitches: stitches as u32,
        });
    }
    Ok(ShapingOut {
        final_stitches: stitches as u32,
        last_line: lines.last().map_or(0, |l| l.line),
        lines,
    })
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
