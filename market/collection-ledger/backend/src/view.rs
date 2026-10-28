//! 台账视图：筛选、分页、汇总、待归还列表与单件往来历史。只读，不修改台账。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::date::parse_date;
use crate::model::*;

/// 距预计归还日不超过这么多天时标为「即将到期」
pub const SOON_DAYS: i64 = 3;
const MAX_PAGE: usize = 100;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// 未归档的全部藏品
    #[default]
    All,
    /// 有在手份数
    InHand,
    /// 有借出未还
    Lent,
    Archived,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub ledger: Ledger,
    pub today: String,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "page")]
    pub limit: usize,
    #[serde(default)]
    pub pending_offset: usize,
    #[serde(default = "page")]
    pub pending_limit: usize,
}

fn page() -> usize {
    20
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    #[serde(flatten)]
    pub item: Item,
    pub available: u32,
    pub outstanding: u32,
    pub active_loans: usize,
    pub has_loans: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DueState {
    Overdue,
    Soon,
    Ok,
    /// 没有预计归还日
    None,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoanView {
    #[serde(flatten)]
    pub loan: Loan,
    pub item_name: String,
    pub returned: u32,
    pub outstanding: u32,
    pub due_state: DueState,
    /// 距预计归还日的天数（负数为已逾期）
    pub days_left: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub items: usize,
    pub archived: usize,
    pub owned: u64,
    pub available: u64,
    pub outstanding: u64,
    pub active_loans: usize,
    pub overdue: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub revision: u64,
    pub items: Vec<ItemView>,
    pub total: usize,
    pub pending: Vec<LoanView>,
    pub pending_total: usize,
    pub summary: Summary,
    pub locations: Vec<String>,
    pub categories: Vec<String>,
    /// 可撤销的最近一步（操作类型）
    pub undo: Option<String>,
}

fn item_view(ledger: &Ledger, item: &Item) -> ItemView {
    ItemView {
        item: item.clone(),
        available: ledger.available(item),
        outstanding: ledger.outstanding(&item.id),
        active_loans: ledger
            .loans
            .iter()
            .filter(|l| l.item_id == item.id && l.outstanding() > 0)
            .count(),
        has_loans: ledger.has_loans(&item.id),
    }
}

pub fn loan_view(ledger: &Ledger, loan: &Loan, today: i64) -> LoanView {
    let days_left = loan
        .due
        .as_deref()
        .and_then(parse_date)
        .map(|due| due - today);
    let outstanding = loan.outstanding();
    let due_state = match days_left {
        _ if outstanding == 0 => DueState::None,
        None => DueState::None,
        Some(d) if d < 0 => DueState::Overdue,
        Some(d) if d <= SOON_DAYS => DueState::Soon,
        Some(_) => DueState::Ok,
    };
    LoanView {
        loan: loan.clone(),
        item_name: ledger
            .item(&loan.item_id)
            .map(|i| i.name.clone())
            .unwrap_or_default(),
        returned: loan.returned(),
        outstanding,
        due_state,
        days_left,
    }
}

fn matches(item: &Item, words: &[String]) -> bool {
    let haystack = [
        &item.name,
        &item.series,
        &item.edition,
        &item.code,
        &item.category,
        &item.location,
        &item.condition,
        &item.notes,
    ]
    .map(|s| s.to_lowercase())
    .join(" ");
    words.iter().all(|w| haystack.contains(w))
}

pub fn view(q: &Query) -> PluginResult<View> {
    let ledger = &q.ledger;
    validate(ledger)?;
    let today = parse_date(&q.today).ok_or_else(|| error("bad_date"))?;
    let words: Vec<String> = q
        .search
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    let limit = q.limit.clamp(1, MAX_PAGE);
    let pending_limit = q.pending_limit.clamp(1, MAX_PAGE);

    let mut list: Vec<ItemView> = ledger
        .items
        .iter()
        .filter(|i| q.location.as_deref().is_none_or(|l| i.location == l))
        .filter(|i| q.category.as_deref().is_none_or(|c| i.category == c))
        .filter(|i| matches(i, &words))
        .map(|i| item_view(ledger, i))
        .filter(|v| match q.state {
            State::All => !v.item.archived,
            State::InHand => !v.item.archived && v.available > 0,
            State::Lent => v.outstanding > 0,
            State::Archived => v.item.archived,
        })
        .collect();
    list.sort_by(|a, b| {
        a.item
            .name
            .to_lowercase()
            .cmp(&b.item.name.to_lowercase())
            .then_with(|| a.item.series.cmp(&b.item.series))
            .then_with(|| a.item.edition.cmp(&b.item.edition))
            .then_with(|| a.item.code.cmp(&b.item.code))
    });
    let total = list.len();
    let items = list.into_iter().skip(q.offset).take(limit).collect();

    // 待归还：有未还份数的借出单，逾期最前，其次按预计归还日，无预计归还日的放最后
    let mut pending: Vec<LoanView> = ledger
        .loans
        .iter()
        .filter(|l| l.outstanding() > 0)
        .map(|l| loan_view(ledger, l, today))
        .collect();
    pending.sort_by(|a, b| {
        let key = |v: &LoanView| (v.days_left.is_none(), v.days_left.unwrap_or(i64::MAX));
        key(a)
            .cmp(&key(b))
            .then_with(|| a.loan.date.cmp(&b.loan.date))
    });
    let pending_total = pending.len();

    let active: Vec<&Item> = ledger.items.iter().filter(|i| !i.archived).collect();
    let summary = Summary {
        items: active.len(),
        archived: ledger.items.len() - active.len(),
        owned: ledger.items.iter().map(|i| u64::from(i.owned)).sum(),
        available: ledger
            .items
            .iter()
            .map(|i| u64::from(ledger.available(i)))
            .sum(),
        outstanding: ledger
            .items
            .iter()
            .map(|i| u64::from(ledger.outstanding(&i.id)))
            .sum(),
        active_loans: pending_total,
        overdue: pending
            .iter()
            .filter(|p| p.due_state == DueState::Overdue)
            .count(),
    };
    let collect = |f: fn(&Item) -> &String| -> Vec<String> {
        ledger
            .items
            .iter()
            .map(f)
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    Ok(View {
        revision: ledger.revision,
        items,
        total,
        pending: pending
            .into_iter()
            .skip(q.pending_offset)
            .take(pending_limit)
            .collect(),
        pending_total,
        summary,
        locations: collect(|i| &i.location),
        categories: collect(|i| &i.category),
        undo: ledger.undo.as_ref().map(|u| u.kind.clone()),
    })
}

/// 单件藏品的完整往来历史（借出单与归还明细，最新在前）
pub fn history(
    ledger: &Ledger,
    item_id: &str,
    today: &str,
) -> PluginResult<(ItemView, Vec<LoanView>)> {
    validate(ledger)?;
    let today = parse_date(today).ok_or_else(|| error("bad_date"))?;
    let item = ledger.item(item_id)?;
    let mut loans: Vec<LoanView> = ledger
        .loans
        .iter()
        .filter(|l| l.item_id == item_id)
        .map(|l| loan_view(ledger, l, today))
        .collect();
    loans.sort_by(|a, b| {
        b.loan
            .date
            .cmp(&a.loan.date)
            .then_with(|| b.loan.id.cmp(&a.loan.id))
    });
    Ok((item_view(ledger, item), loans))
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
