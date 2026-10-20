//! 分账核心：金额一律以「分」为最小单位的整数计算，禁止浮点。
//! 余分按公开、稳定的成员顺序分配；校验失败返回错误码，由前端翻译。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::PluginError;

/// 单笔金额上限（分）：10 亿元
pub const MAX_CENTS: i64 = 100_000_000_000;
pub const MAX_MEMBERS: usize = 64;
pub const MAX_EXPENSES: usize = 1000;
pub const MAX_SETTLEMENTS: usize = 1000;
/// 单个成员的最大份数
pub const MAX_SHARES: u64 = 100_000_000;

/// 诊断风格的输入错误：code + params 交由前端翻译
pub fn err(code: &str) -> PluginError {
    PluginError::new(code)
}

pub fn errp(code: &str, key: &str, value: impl Into<Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Weight {
    pub id: String,
    pub shares: u64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    pub id: String,
    /// 精确金额（分），十进制字符串避免前端浮点
    pub cents: String,
}

/// 分摊方式：均分 / 按份数 / 逐人精确金额
#[derive(Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Split {
    Equal { participants: Vec<String> },
    Shares { weights: Vec<Weight> },
    Exact { amounts: Vec<Amount> },
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Expense {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub date: Option<String>,
    /// 总额（元的十进制字符串，如 "12.34"），Rust 侧解析为分
    pub total: String,
    pub payer: String,
    pub split: Split,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settlement {
    pub id: String,
    pub from: String,
    pub to: String,
    /// 金额（元的十进制字符串）
    pub cents: String,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    pub members: Vec<Member>,
    pub expenses: Vec<Expense>,
    pub settlements: Vec<Settlement>,
}

// ---- 输出 ----

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub id: String,
    pub cents: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseSplit {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub payer: String,
    pub total_cents: i64,
    pub allocations: Vec<Allocation>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub id: String,
    pub name: String,
    pub paid: i64,
    pub owed: i64,
    pub settled_out: i64,
    pub settled_in: i64,
    /// 应收为正、应补为负；全体之和恒为 0
    pub net: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub from: String,
    pub to: String,
    pub cents: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettlementRecord {
    pub id: String,
    pub from: String,
    pub to: String,
    /// 已校验的金额（分）
    pub cents: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub expenses: Vec<ExpenseSplit>,
    #[serde(default)]
    pub settlements: Vec<SettlementRecord>,
    pub balances: Vec<Balance>,
    pub transfers: Vec<Transfer>,
    pub total_cents: i64,
}

/// "12.34" → 1234 分；只允许非负、至多两位小数
pub fn parse_cents(text: &str, code: &str) -> Result<i64, PluginError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(err(code));
    }
    let (yuan, frac) = match text.split_once('.') {
        Some((yuan, frac)) => (yuan, frac),
        None => (text, ""),
    };
    if frac.len() > 2 || yuan.is_empty() && frac.is_empty() {
        return Err(errp(code, "value", text));
    }
    if !yuan.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
        || (yuan.is_empty() && frac.is_empty())
    {
        return Err(errp(code, "value", text));
    }
    let yuan_value: i64 = yuan.parse().map_err(|_| errp(code, "value", text))?;
    let mut cents = yuan_value
        .checked_mul(100)
        .ok_or_else(|| errp(code, "value", text))?;
    let mut scale = 100;
    for digit in frac.chars() {
        scale /= 10;
        cents += (digit as i64 - '0' as i64) * scale;
    }
    if cents > MAX_CENTS {
        return Err(errp("esp.too_large", "max", MAX_CENTS));
    }
    Ok(cents)
}

/// 按成员列表顺序排序键，保证余分分配顺序公开、稳定
fn member_order(members: &[Member], id: &str) -> Option<usize> {
    members.iter().position(|m| m.id == id)
}

fn require_member(members: &[Member], id: &str) -> Result<usize, PluginError> {
    member_order(members, id).ok_or_else(|| errp("esp.unknown_member", "id", id))
}

fn check_ids_unique(entries: &[String], code: &str) -> Result<(), PluginError> {
    for (index, id) in entries.iter().enumerate() {
        if entries[..index].contains(id) {
            return Err(errp(code, "id", id.as_str()));
        }
    }
    Ok(())
}

/// 单笔费用的分摊：返回按成员顺序排列的 (id, cents)，余额守恒（和恒等于总额）
pub fn allocate(
    members: &[Member],
    expense: &Expense,
) -> Result<(i64, Vec<Allocation>), PluginError> {
    let total = parse_cents(&expense.total, "esp.bad_total")?;
    require_member(members, &expense.payer)?;
    // 参与者统一转成按成员顺序的 (id, 份数, 显式金额)
    let participants: Vec<(String, u64, Option<i64>)> = match &expense.split {
        Split::Equal { participants } => {
            check_ids_unique(participants, "esp.duplicate_entry")?;
            if participants.is_empty() {
                return Err(err("esp.empty_participants"));
            }
            participants
                .iter()
                .map(|id| {
                    require_member(members, id)?;
                    Ok((id.clone(), 1, None))
                })
                .collect::<Result<Vec<_>, PluginError>>()?
        }
        Split::Shares { weights } => {
            check_ids_unique(
                &weights.iter().map(|w| w.id.clone()).collect::<Vec<_>>(),
                "esp.duplicate_entry",
            )?;
            if weights.is_empty() || weights.iter().all(|w| w.shares == 0) {
                return Err(err("esp.shares_empty"));
            }
            weights
                .iter()
                .map(|weight| {
                    require_member(members, &weight.id)?;
                    if weight.shares > MAX_SHARES {
                        return Err(errp("esp.shares_too_large", "max", MAX_SHARES));
                    }
                    Ok((weight.id.clone(), weight.shares, None))
                })
                .collect::<Result<Vec<_>, PluginError>>()?
        }
        Split::Exact { amounts } => {
            check_ids_unique(
                &amounts.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
                "esp.duplicate_entry",
            )?;
            if amounts.is_empty() {
                return Err(err("esp.empty_participants"));
            }
            let mut exact_sum = 0i64;
            let parsed = amounts
                .iter()
                .map(|amount| {
                    require_member(members, &amount.id)?;
                    let cents = parse_cents(&amount.cents, "esp.bad_amount")?;
                    exact_sum += cents;
                    Ok((amount.id.clone(), 0u64, Some(cents)))
                })
                .collect::<Result<Vec<_>, PluginError>>()?;
            if exact_sum != total {
                return Err(errp("esp.exact_sum", "expected", total).with("actual", exact_sum));
            }
            parsed
        }
    };

    // 精确模式直接输出
    if participants.iter().all(|(_, _, exact)| exact.is_some()) {
        let allocations = participants
            .iter()
            .map(|(id, _, exact)| Allocation {
                id: id.clone(),
                cents: exact.unwrap_or_default(),
            })
            .collect();
        return Ok((total, allocations));
    }

    // 均分 / 按份数：unit + 余分按成员顺序分配（按份数时余分跟随份数）
    let total_shares: u64 = participants.iter().map(|(_, shares, _)| shares).sum();
    if total_shares == 0 {
        return Err(err("esp.shares_empty"));
    }
    let unit = total / total_shares as i64;
    let mut remainder = total - unit * total_shares as i64;
    let mut allocations = Vec::with_capacity(participants.len());
    for (id, shares, _exact) in &participants {
        let mut cents = unit * *shares as i64;
        // 余分：按成员顺序，每人先补 min(份数, 剩余) 分
        let extra = if remainder > 0 {
            let want = (*shares).min(remainder as u64) as i64;
            want.min(remainder)
        } else {
            0
        };
        cents += extra;
        remainder -= extra;
        allocations.push(Allocation {
            id: id.clone(),
            cents,
        });
    }
    if remainder != 0 {
        return Err(err("esp.remainder"));
    }
    Ok((total, allocations))
}

/// 全量结算：逐笔分摊 + 成员余额 + 贪心转账建议（余额守恒）
pub fn settle(ledger: &Ledger) -> Result<Report, PluginError> {
    if ledger.members.is_empty() {
        return Err(err("esp.no_members"));
    }
    if ledger.members.len() > MAX_MEMBERS {
        return Err(errp("esp.too_many_members", "max", MAX_MEMBERS));
    }
    // 成员 id 重复 = 同名冲突的根源，必须拒绝
    check_ids_unique(
        &ledger
            .members
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>(),
        "esp.duplicate_member",
    )?;
    if ledger.expenses.len() > MAX_EXPENSES {
        return Err(errp("esp.too_many_expenses", "max", MAX_EXPENSES));
    }
    if ledger.settlements.len() > MAX_SETTLEMENTS {
        return Err(errp("esp.too_many_settlements", "max", MAX_SETTLEMENTS));
    }
    let mut settlements = Vec::with_capacity(ledger.settlements.len());
    for settlement in &ledger.settlements {
        require_member(&ledger.members, &settlement.from)?;
        require_member(&ledger.members, &settlement.to)?;
        if settlement.from == settlement.to {
            return Err(err("esp.self_settlement"));
        }
        let cents = parse_cents(&settlement.cents, "esp.bad_total")?;
        if cents <= 0 {
            return Err(err("esp.settlement_positive"));
        }
        settlements.push(SettlementRecord {
            id: settlement.id.clone(),
            from: settlement.from.clone(),
            to: settlement.to.clone(),
            cents,
            date: settlement.date.clone(),
        });
    }

    let mut expense_splits = Vec::with_capacity(ledger.expenses.len());
    let mut paid: Vec<i64> = vec![0; ledger.members.len()];
    let mut owed: Vec<i64> = vec![0; ledger.members.len()];
    let mut total_cents = 0i64;
    for expense in &ledger.expenses {
        let (total, allocations) = allocate(&ledger.members, expense)?;
        total_cents += total;
        let payer_index = require_member(&ledger.members, &expense.payer)?;
        paid[payer_index] += total;
        for allocation in &allocations {
            let index = require_member(&ledger.members, &allocation.id)?;
            owed[index] += allocation.cents;
        }
        expense_splits.push(ExpenseSplit {
            id: expense.id.clone(),
            title: expense.title.clone(),
            date: expense.date.clone(),
            payer: expense.payer.clone(),
            total_cents: total,
            allocations,
        });
    }

    let mut settled_out = vec![0i64; ledger.members.len()];
    let mut settled_in = vec![0i64; ledger.members.len()];
    for settlement in &settlements {
        let from = require_member(&ledger.members, &settlement.from)?;
        let to = require_member(&ledger.members, &settlement.to)?;
        settled_out[from] += settlement.cents;
        settled_in[to] += settlement.cents;
    }

    let balances: Vec<Balance> = ledger
        .members
        .iter()
        .enumerate()
        .map(|(index, member)| Balance {
            id: member.id.clone(),
            name: member.name.clone(),
            paid: paid[index],
            owed: owed[index],
            settled_out: settled_out[index],
            settled_in: settled_in[index],
            net: paid[index] - owed[index] - settled_out[index] + settled_in[index],
        })
        .collect();

    // 贪心配对：应收最多者优先匹配应补最多者；减少转账次数但不宣称全局最少
    let mut creditors: Vec<(usize, i64)> = balances
        .iter()
        .enumerate()
        .filter(|(_, balance)| balance.net > 0)
        .map(|(index, balance)| (index, balance.net))
        .collect();
    let mut debtors: Vec<(usize, i64)> = balances
        .iter()
        .enumerate()
        .filter(|(_, balance)| balance.net < 0)
        .map(|(index, balance)| (index, -balance.net))
        .collect();
    creditors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    debtors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut transfers = Vec::new();
    let mut credit_at = 0usize;
    let mut debit_at = 0usize;
    while credit_at < creditors.len() && debit_at < debtors.len() {
        let amount = creditors[credit_at].1.min(debtors[debit_at].1);
        if amount > 0 {
            transfers.push(Transfer {
                from: ledger.members[debtors[debit_at].0].id.clone(),
                to: ledger.members[creditors[credit_at].0].id.clone(),
                cents: amount,
            });
        }
        creditors[credit_at].1 -= amount;
        debtors[debit_at].1 -= amount;
        if creditors[credit_at].1 == 0 {
            credit_at += 1;
        }
        if debtors[debit_at].1 == 0 {
            debit_at += 1;
        }
    }

    Ok(Report {
        expenses: expense_splits,
        settlements,
        balances,
        transfers,
        total_cents,
    })
}

/// 金额格式化：分 → "12.34"；负数带符号
pub fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

#[cfg(test)]
#[path = "ledger_test.rs"]
mod tests;
