//! 台账数据结构、守恒规则与上限。
//!
//! 守恒式（对每件藏品）：在手量 + 未归还量 = 保有量。
//! - 未归还量 = 该藏品所有借出单的（借出份数 − 已归还份数）之和。
//! - 每张借出单的已归还份数不超过借出份数；在手量不小于 0。
//! - 借出单与归还明细只追加、不改写，构成完整往来历史；只有「撤销最近一步」能移除刚刚加入的记录。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::parse_date;

/// 存档格式版本
pub const SCHEMA: u32 = 1;
pub const MAX_ITEMS: usize = 500;
pub const MAX_LOANS: usize = 3000;
pub const MAX_RETURNS_PER_LOAN: usize = 50;
/// 单件藏品保有量上限
pub const MAX_OWNED: u32 = 100_000;
/// 文本字段长度上限（字符）
pub const MAX_NAME: usize = 80;
pub const MAX_TEXT: usize = 500;
/// 序列化后的台账上限，低于宿主单个状态键 2 MiB
pub const MAX_LEDGER_BYTES: usize = 1_800_000;
/// 记住最近处理过的操作编号，用于识别重复提交
pub const RECENT_OPS: usize = 100;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("ledger.{code}"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Ledger {
    pub schema: u32,
    /// 每次成功修改加 1，用于拒绝基于旧数据的提交
    pub revision: u64,
    pub items: Vec<Item>,
    pub loans: Vec<Loan>,
    /// 最近处理过的操作编号（重复提交只生效一次）
    pub recent_ops: Vec<String>,
    /// 最近一步的撤销信息
    pub undo: Option<Undo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// 系列 / 版次：同名不同版本靠它们区分
    pub series: String,
    pub edition: String,
    /// 用户自定义编号
    pub code: String,
    pub category: String,
    pub location: String,
    pub condition: String,
    pub notes: String,
    /// 保有量（在手 + 借出未还）
    pub owned: u32,
    pub archived: bool,
    pub has_photo: bool,
    pub created: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Loan {
    pub id: String,
    pub item_id: String,
    /// 借用人昵称（只记昵称，不关联通讯录）
    pub borrower: String,
    pub qty: u32,
    pub date: String,
    /// 预计归还日（可选，只用于本地列表标记）
    pub due: Option<String>,
    pub note: String,
    pub returns: Vec<Return>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Return {
    pub id: String,
    pub qty: u32,
    pub date: String,
    pub note: String,
}

/// 撤销最近一步：受影响的藏品与借出单的旧版本（None 表示操作前不存在）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Undo {
    /// 操作类型（界面显示「撤销借出」之类）
    pub kind: String,
    pub item_id: Option<String>,
    pub item: Option<Item>,
    pub loan_id: Option<String>,
    pub loan: Option<Loan>,
}

impl Loan {
    pub fn returned(&self) -> u32 {
        self.returns.iter().map(|r| r.qty).sum()
    }

    pub fn outstanding(&self) -> u32 {
        self.qty.saturating_sub(self.returned())
    }
}

impl Ledger {
    pub fn item(&self, id: &str) -> PluginResult<&Item> {
        self.items
            .iter()
            .find(|i| i.id == id)
            .ok_or_else(|| error("item_not_found"))
    }

    pub fn loan(&self, id: &str) -> PluginResult<&Loan> {
        self.loans
            .iter()
            .find(|l| l.id == id)
            .ok_or_else(|| error("loan_not_found"))
    }

    /// 某件藏品尚未归还的总份数
    pub fn outstanding(&self, item_id: &str) -> u32 {
        self.loans
            .iter()
            .filter(|l| l.item_id == item_id)
            .map(Loan::outstanding)
            .sum()
    }

    /// 在手（可借出）份数
    pub fn available(&self, item: &Item) -> u32 {
        item.owned.saturating_sub(self.outstanding(&item.id))
    }

    pub fn has_loans(&self, item_id: &str) -> bool {
        self.loans.iter().any(|l| l.item_id == item_id)
    }
}

fn text_ok(text: &str, max: usize) -> bool {
    text.chars().count() <= max && !text.chars().any(|c| c.is_control() && c != '\n')
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// 藏品字段检查（新增、编辑、恢复共用）
pub fn check_item(item: &Item) -> PluginResult<()> {
    if !id_ok(&item.id) || parse_date(&item.created).is_none() {
        return Err(error("bad_item"));
    }
    if item.name.trim().is_empty() || !text_ok(&item.name, MAX_NAME) {
        return Err(error("bad_name"));
    }
    for (text, max) in [
        (&item.series, MAX_NAME),
        (&item.edition, MAX_NAME),
        (&item.code, MAX_NAME),
        (&item.category, MAX_NAME),
        (&item.location, MAX_NAME),
        (&item.condition, MAX_NAME),
        (&item.notes, MAX_TEXT),
    ] {
        if !text_ok(text, max) {
            return Err(error("text_too_long"));
        }
    }
    if item.owned > MAX_OWNED {
        return Err(error("bad_qty").with("max", MAX_OWNED));
    }
    Ok(())
}

/// 完整校验：结构、引用关系、日期与守恒
pub fn validate(ledger: &Ledger) -> PluginResult<()> {
    if ledger.schema != SCHEMA {
        return Err(error("bad_schema"));
    }
    if ledger.items.len() > MAX_ITEMS || ledger.loans.len() > MAX_LOANS {
        return Err(error("too_many"));
    }
    let mut ids = BTreeSet::new();
    for item in &ledger.items {
        check_item(item)?;
        if !ids.insert(item.id.as_str()) {
            return Err(error("duplicate_id"));
        }
    }
    for loan in &ledger.loans {
        ledger
            .item(&loan.item_id)
            .map_err(|_| error("broken_link"))?;
        if !id_ok(&loan.id) || !ids.insert(loan.id.as_str()) {
            return Err(error("duplicate_id"));
        }
        let start = parse_date(&loan.date).ok_or_else(|| error("bad_date"))?;
        if let Some(due) = &loan.due {
            let due = parse_date(due).ok_or_else(|| error("bad_date"))?;
            if due < start {
                return Err(error("due_before_loan"));
            }
        }
        if loan.qty == 0
            || loan.borrower.trim().is_empty()
            || !text_ok(&loan.borrower, MAX_NAME)
            || !text_ok(&loan.note, MAX_TEXT)
        {
            return Err(error("bad_loan"));
        }
        if loan.returns.len() > MAX_RETURNS_PER_LOAN {
            return Err(error("too_many"));
        }
        let mut returned = 0u32;
        for r in &loan.returns {
            let day = parse_date(&r.date).ok_or_else(|| error("bad_date"))?;
            if r.qty == 0
                || day < start
                || !id_ok(&r.id)
                || !ids.insert(r.id.as_str())
                || !text_ok(&r.note, MAX_TEXT)
            {
                return Err(error("bad_return"));
            }
            returned = returned.saturating_add(r.qty);
        }
        if returned > loan.qty {
            return Err(error("over_return"));
        }
    }
    // 守恒：每件藏品的未归还量不超过保有量
    for item in &ledger.items {
        if ledger.outstanding(&item.id) > item.owned {
            return Err(error("conservation"));
        }
    }
    if ledger.recent_ops.len() > RECENT_OPS {
        return Err(error("bad_ledger"));
    }
    Ok(())
}

/// 序列化大小检查
pub fn check_size(ledger: &Ledger) -> PluginResult<()> {
    let bytes = serde_json::to_vec(ledger)
        .map_err(|_| error("bad_ledger"))?
        .len();
    if bytes > MAX_LEDGER_BYTES {
        return Err(error("ledger_full"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "model_test.rs"]
mod tests;
