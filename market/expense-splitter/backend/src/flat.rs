//! 命令行 / Agent 用的扁平入口：用一行文字描述一笔费用，直接算出每人余额与转账建议。
//!
//! 费用写法：`付款人:金额[:参与者|参与者*份数]`。省略参与者时由所有人均摊
//! （所有人 = members 与费用里出现过的名字，按出现顺序）；`名字*份数` 表示按份数分摊，
//! 不写份数为 1 份。已转账写法：`付款方>收款方:金额`。
//!
//! 内部转换为界面同一套账本后调用 ledger::settle，金额按「分」整数计算、余额守恒。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::ledger::{
    Expense, Ledger, MAX_EXPENSES, MAX_MEMBERS, Member, Settlement, Split, Weight, err, errp,
    format_cents, settle,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitArgs {
    pub expenses: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub settled: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub from: String,
    pub to: String,
    pub amount: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BalanceRow {
    pub name: String,
    pub paid: String,
    pub share: String,
    /// 正数应收，负数应付
    pub net: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitOut {
    pub transfers: Vec<Row>,
    pub balances: Vec<BalanceRow>,
    pub total: String,
}

fn name(text: &str) -> PluginResult<String> {
    let name = text.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(errp("esp.bad_line", "value", text));
    }
    Ok(name.to_string())
}

fn remember(all: &mut Vec<String>, who: &str) {
    if !all.iter().any(|m| m == who) {
        all.push(who.to_string());
    }
}

/// 解析参与者：`A|B*2|C`
fn participants(text: &str) -> PluginResult<Vec<(String, u64)>> {
    text.split('|')
        .map(|part| {
            let (who, shares) = match part.split_once('*') {
                Some((who, shares)) => (
                    who,
                    shares
                        .trim()
                        .parse::<u64>()
                        .map_err(|_| errp("esp.bad_line", "value", part))?,
                ),
                None => (part, 1),
            };
            Ok((name(who)?, shares))
        })
        .collect()
}

pub fn split(args: SplitArgs) -> PluginResult<SplitOut> {
    if args.expenses.is_empty() {
        return Err(err("esp.no_expenses"));
    }
    if args.expenses.len() > MAX_EXPENSES {
        return Err(errp("esp.too_many_expenses", "max", MAX_EXPENSES));
    }
    let mut all: Vec<String> = Vec::new();
    for m in &args.members {
        remember(&mut all, &name(m)?);
    }
    // 先解析一遍，收集所有出现过的名字
    let mut parsed = Vec::new();
    for line in &args.expenses {
        let mut parts = line.splitn(3, ':');
        let payer = name(parts.next().unwrap_or_default())?;
        let total = parts
            .next()
            .ok_or_else(|| errp("esp.bad_line", "value", line.as_str()))?
            .trim()
            .to_string();
        let who = match parts.next() {
            Some(list) if !list.trim().is_empty() => Some(participants(list)?),
            _ => None,
        };
        remember(&mut all, &payer);
        for (p, _) in who.iter().flatten() {
            remember(&mut all, p);
        }
        parsed.push((payer, total, who));
    }
    let mut settled = Vec::new();
    for line in &args.settled {
        let (pair, cents) = line
            .split_once(':')
            .ok_or_else(|| errp("esp.bad_line", "value", line.as_str()))?;
        let (from, to) = pair
            .split_once('>')
            .ok_or_else(|| errp("esp.bad_line", "value", line.as_str()))?;
        let (from, to) = (name(from)?, name(to)?);
        remember(&mut all, &from);
        remember(&mut all, &to);
        settled.push((from, to, cents.trim().to_string()));
    }
    if all.len() > MAX_MEMBERS {
        return Err(errp("esp.too_many_members", "max", MAX_MEMBERS));
    }
    // 名字即编号：每个名字只出现一次，同名视为同一人
    let ledger = Ledger {
        members: all
            .iter()
            .map(|n| Member {
                id: n.clone(),
                name: n.clone(),
            })
            .collect(),
        expenses: parsed
            .into_iter()
            .enumerate()
            .map(|(i, (payer, total, who))| Expense {
                id: format!("e{i}"),
                title: String::new(),
                date: None,
                total,
                payer,
                split: match who {
                    None => Split::Equal {
                        participants: all.clone(),
                    },
                    Some(list) if list.iter().all(|(_, s)| *s == 1) => Split::Equal {
                        participants: list.into_iter().map(|(n, _)| n).collect(),
                    },
                    Some(list) => Split::Shares {
                        weights: list
                            .into_iter()
                            .map(|(id, shares)| Weight { id, shares })
                            .collect(),
                    },
                },
            })
            .collect(),
        settlements: settled
            .into_iter()
            .enumerate()
            .map(|(i, (from, to, cents))| Settlement {
                id: format!("s{i}"),
                from,
                to,
                cents,
                date: None,
            })
            .collect(),
    };
    let report = settle(&ledger)?;
    Ok(SplitOut {
        transfers: report
            .transfers
            .iter()
            .map(|t| Row {
                from: t.from.clone(),
                to: t.to.clone(),
                amount: format_cents(t.cents),
            })
            .collect(),
        balances: report
            .balances
            .iter()
            .map(|b| BalanceRow {
                name: b.name.clone(),
                paid: format_cents(b.paid),
                share: format_cents(b.owed),
                net: format_cents(b.net),
            })
            .collect(),
        total: format_cents(report.total_cents),
    })
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
