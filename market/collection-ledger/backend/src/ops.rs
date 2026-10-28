//! 台账操作：新增/编辑藏品、调整保有量、借出、归还、归档、删除、照片标记与撤销。
//!
//! 每次提交都带操作编号与界面看到的版本号：
//! - 同一操作编号重复提交只生效一次（第二次原样返回当前台账）；
//! - 版本号与当前台账不符说明界面数据已过期，直接拒绝，不在旧数据上叠加修改；
//! - `preview` 只计算会影响的数量，不产生新台账，供界面在确认前展示。
//!
//! 只保留最近一步的撤销：记录受影响藏品与借出单的旧版本，撤销时原样放回。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::date::parse_date;
use crate::model::*;

/// 藏品资料（新增与编辑共用；保有量在新增时设定，之后通过「调整数量」修改）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fields {
    pub name: String,
    pub series: String,
    pub edition: String,
    pub code: String,
    pub category: String,
    pub location: String,
    pub condition: String,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Op {
    #[serde(rename_all = "camelCase")]
    Create {
        fields: Fields,
        owned: u32,
    },
    #[serde(rename_all = "camelCase")]
    Edit {
        item_id: String,
        fields: Fields,
    },
    #[serde(rename_all = "camelCase")]
    SetOwned {
        item_id: String,
        owned: u32,
    },
    #[serde(rename_all = "camelCase")]
    Lend {
        item_id: String,
        borrower: String,
        qty: u32,
        date: String,
        #[serde(default)]
        due: Option<String>,
        #[serde(default)]
        note: String,
    },
    #[serde(rename_all = "camelCase")]
    Return {
        loan_id: String,
        qty: u32,
        date: String,
        #[serde(default)]
        note: String,
    },
    #[serde(rename_all = "camelCase")]
    Archive {
        item_id: String,
        archived: bool,
    },
    /// 只能删除从未借出过的藏品（不可撤销，界面需二次确认）
    #[serde(rename_all = "camelCase")]
    Delete {
        item_id: String,
    },
    /// 照片已由界面写入/删除，这里只更新标记（不可撤销）
    #[serde(rename_all = "camelCase")]
    SetPhoto {
        item_id: String,
        has_photo: bool,
    },
    Undo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub ledger: Ledger,
    pub op: Op,
    /// 界面生成的唯一操作编号
    pub op_id: String,
    /// 界面看到的台账版本
    pub revision: u64,
    /// 本地今天（YYYY-MM-DD），借出与归还日期不能晚于今天
    pub today: String,
    #[serde(default)]
    pub preview: bool,
}

/// 一次操作对数量的影响（确认前展示）
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Impact {
    pub kind: String,
    pub item_id: Option<String>,
    pub item_name: String,
    pub owned: [u32; 2],
    pub available: [u32; 2],
    pub outstanding: [u32; 2],
    /// 借出单的未归还量变化（借出、归还时）
    pub loan_outstanding: Option<[u32; 2]>,
    pub borrower: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// 预览时为 None
    pub ledger: Option<Ledger>,
    pub impact: Impact,
    /// 重复提交：没有再次修改
    pub duplicate: bool,
    pub created_id: Option<String>,
}

/// 由操作编号派生实体编号：操作编号全局唯一，所以合并不同备份时编号不会撞车
fn derive_id(prefix: char, op_id: &str) -> String {
    let core: String = op_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(24)
        .collect();
    format!("{prefix}-{core}")
}

fn clean(text: &str) -> String {
    text.trim().to_string()
}

fn item_from(fields: &Fields, id: String, owned: u32, created: String) -> Item {
    Item {
        id,
        name: clean(&fields.name),
        series: clean(&fields.series),
        edition: clean(&fields.edition),
        code: clean(&fields.code),
        category: clean(&fields.category),
        location: clean(&fields.location),
        condition: clean(&fields.condition),
        notes: fields.notes.trim().to_string(),
        owned,
        archived: false,
        has_photo: false,
        created,
    }
}

fn day(text: &str) -> PluginResult<i64> {
    parse_date(text).ok_or_else(|| error("bad_date"))
}

/// 某件藏品的 [保有, 在手, 未还]
fn counts(ledger: &Ledger, item_id: &str) -> [u32; 3] {
    match ledger.item(item_id) {
        Ok(item) => [
            item.owned,
            ledger.available(item),
            ledger.outstanding(item_id),
        ],
        Err(_) => [0, 0, 0],
    }
}

fn replace_item(ledger: &mut Ledger, item: Item) {
    match ledger.items.iter_mut().find(|i| i.id == item.id) {
        Some(slot) => *slot = item,
        None => ledger.items.push(item),
    }
}

fn replace_loan(ledger: &mut Ledger, loan: Loan) {
    match ledger.loans.iter_mut().find(|l| l.id == loan.id) {
        Some(slot) => *slot = loan,
        None => ledger.loans.push(loan),
    }
}

fn kind_of(op: &Op) -> &'static str {
    match op {
        Op::Create { .. } => "create",
        Op::Edit { .. } => "edit",
        Op::SetOwned { .. } => "setOwned",
        Op::Lend { .. } => "lend",
        Op::Return { .. } => "return",
        Op::Archive { .. } => "archive",
        Op::Delete { .. } => "delete",
        Op::SetPhoto { .. } => "setPhoto",
        Op::Undo => "undo",
    }
}

pub fn apply(req: Request) -> PluginResult<Outcome> {
    let Request {
        ledger,
        op,
        op_id,
        revision,
        today,
        preview,
    } = req;
    validate(&ledger)?;
    if op_id.is_empty() || op_id.len() > 64 {
        return Err(error("bad_op"));
    }
    if ledger.recent_ops.contains(&op_id) {
        // 重复提交：不再修改，返回当前台账
        return Ok(Outcome {
            ledger: Some(ledger),
            impact: Impact {
                kind: kind_of(&op).to_string(),
                ..Impact::default()
            },
            duplicate: true,
            created_id: None,
        });
    }
    if revision != ledger.revision {
        return Err(error("stale"));
    }
    let today = day(&today)?;
    let kind = kind_of(&op).to_string();
    let mut next = ledger.clone();
    // 撤销信息：受影响的藏品 / 借出单旧版本
    let mut undo = Some(Undo {
        kind: kind.clone(),
        ..Undo::default()
    });
    let mut touched_item: Option<String> = None;
    let mut touched_loan: Option<String> = None;
    let mut created_id = None;

    match op {
        Op::Create { fields, owned } => {
            if next.items.len() >= MAX_ITEMS {
                return Err(error("too_many").with("max", MAX_ITEMS));
            }
            if owned == 0 {
                return Err(error("bad_qty").with("max", MAX_OWNED));
            }
            let id = derive_id('i', &op_id);
            if next.items.iter().any(|i| i.id == id) {
                return Err(error("bad_op"));
            }
            let item = item_from(&fields, id.clone(), owned, crate::date::format_date(today));
            check_item(&item)?;
            next.items.push(item);
            touched_item = Some(id.clone());
            created_id = Some(id);
        }
        Op::Edit { item_id, fields } => {
            let old = next.item(&item_id)?.clone();
            let mut item = item_from(&fields, old.id.clone(), old.owned, old.created.clone());
            item.archived = old.archived;
            item.has_photo = old.has_photo;
            check_item(&item)?;
            replace_item(&mut next, item);
            touched_item = Some(item_id);
        }
        Op::SetOwned { item_id, owned } => {
            let mut item = next.item(&item_id)?.clone();
            let outstanding = next.outstanding(&item_id);
            if owned < outstanding {
                return Err(error("below_outstanding").with("outstanding", outstanding));
            }
            if owned > MAX_OWNED {
                return Err(error("bad_qty").with("max", MAX_OWNED));
            }
            item.owned = owned;
            replace_item(&mut next, item);
            touched_item = Some(item_id);
        }
        Op::Lend {
            item_id,
            borrower,
            qty,
            date,
            due,
            note,
        } => {
            let item = next.item(&item_id)?.clone();
            if item.archived {
                return Err(error("archived"));
            }
            let available = next.available(&item);
            if qty == 0 || qty > available {
                return Err(error("over_available").with("available", available));
            }
            let start = day(&date)?;
            if start > today {
                return Err(error("future_date"));
            }
            let due = due.filter(|d| !d.trim().is_empty());
            if let Some(due) = &due
                && day(due)? < start
            {
                return Err(error("due_before_loan"));
            }
            if next.loans.len() >= MAX_LOANS {
                return Err(error("too_many").with("max", MAX_LOANS));
            }
            let id = derive_id('l', &op_id);
            let loan = Loan {
                id: id.clone(),
                item_id: item_id.clone(),
                borrower: clean(&borrower),
                qty,
                date,
                due,
                note: note.trim().to_string(),
                returns: Vec::new(),
            };
            next.loans.push(loan);
            touched_item = Some(item_id);
            touched_loan = Some(id.clone());
            created_id = Some(id);
        }
        Op::Return {
            loan_id,
            qty,
            date,
            note,
        } => {
            let mut loan = next.loan(&loan_id)?.clone();
            let outstanding = loan.outstanding();
            if qty == 0 || qty > outstanding {
                return Err(error("over_return").with("outstanding", outstanding));
            }
            let when = day(&date)?;
            if when > today {
                return Err(error("future_date"));
            }
            if when < day(&loan.date)? {
                return Err(error("return_before_loan"));
            }
            if loan.returns.len() >= MAX_RETURNS_PER_LOAN {
                return Err(error("too_many").with("max", MAX_RETURNS_PER_LOAN));
            }
            let id = derive_id('r', &op_id);
            loan.returns.push(Return {
                id: id.clone(),
                qty,
                date,
                note: note.trim().to_string(),
            });
            touched_item = Some(loan.item_id.clone());
            touched_loan = Some(loan_id);
            replace_loan(&mut next, loan);
            created_id = Some(id);
        }
        Op::Archive { item_id, archived } => {
            let mut item = next.item(&item_id)?.clone();
            if archived && next.outstanding(&item_id) > 0 {
                return Err(error("archive_lent"));
            }
            item.archived = archived;
            replace_item(&mut next, item);
            touched_item = Some(item_id);
        }
        Op::Delete { item_id } => {
            next.item(&item_id)?;
            if next.has_loans(&item_id) {
                return Err(error("delete_with_loans"));
            }
            next.items.retain(|i| i.id != item_id);
            touched_item = Some(item_id);
            undo = None;
        }
        Op::SetPhoto { item_id, has_photo } => {
            let mut item = next.item(&item_id)?.clone();
            item.has_photo = has_photo;
            replace_item(&mut next, item);
            touched_item = Some(item_id);
            undo = None;
        }
        Op::Undo => {
            let step = next.undo.clone().ok_or_else(|| error("nothing_to_undo"))?;
            if let Some(id) = &step.loan_id {
                match step.loan.clone() {
                    Some(loan) => replace_loan(&mut next, loan),
                    None => next.loans.retain(|l| &l.id != id),
                }
                touched_loan = Some(id.clone());
            }
            if let Some(id) = &step.item_id {
                match step.item.clone() {
                    Some(item) => replace_item(&mut next, item),
                    None => next.items.retain(|i| &i.id != id),
                }
                touched_item = Some(id.clone());
            }
            undo = None;
        }
    }

    // 影响：操作前后的数量
    let item_id = touched_item.clone();
    let before = item_id
        .as_deref()
        .map(|id| counts(&ledger, id))
        .unwrap_or_default();
    let after = item_id
        .as_deref()
        .map(|id| counts(&next, id))
        .unwrap_or_default();
    let loan_out = |l: &Ledger| {
        touched_loan
            .as_deref()
            .and_then(|id| l.loan(id).ok())
            .map(Loan::outstanding)
            .unwrap_or(0)
    };
    let impact = Impact {
        kind: kind.clone(),
        item_id: item_id.clone(),
        item_name: item_id
            .as_deref()
            .and_then(|id| next.item(id).or_else(|_| ledger.item(id)).ok())
            .map(|i| i.name.clone())
            .unwrap_or_default(),
        owned: [before[0], after[0]],
        available: [before[1], after[1]],
        outstanding: [before[2], after[2]],
        loan_outstanding: touched_loan
            .as_ref()
            .map(|_| [loan_out(&ledger), loan_out(&next)]),
        borrower: touched_loan
            .as_deref()
            .and_then(|id| next.loan(id).or_else(|_| ledger.loan(id)).ok())
            .map(|l| l.borrower.clone()),
    };

    // 修改后的台账必须仍然满足全部规则（守恒、引用、日期）
    validate(&next)?;
    if preview {
        return Ok(Outcome {
            ledger: None,
            impact,
            duplicate: false,
            created_id: None,
        });
    }
    if let Some(step) = undo.as_mut() {
        step.item_id = touched_item.clone();
        step.item = touched_item
            .as_deref()
            .and_then(|id| ledger.item(id).ok().cloned());
        step.loan_id = touched_loan.clone();
        step.loan = touched_loan
            .as_deref()
            .and_then(|id| ledger.loan(id).ok().cloned());
    }
    next.undo = undo;
    next.revision = ledger.revision + 1;
    next.recent_ops.push(op_id);
    if next.recent_ops.len() > RECENT_OPS {
        next.recent_ops.remove(0);
    }
    check_size(&next)?;
    Ok(Outcome {
        ledger: Some(next),
        impact,
        duplicate: false,
        created_id,
    })
}

#[cfg(test)]
#[path = "ops_test.rs"]
pub(crate) mod tests;
