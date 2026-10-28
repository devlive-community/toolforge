//! 导出与恢复：清单 CSV、完整 JSON 备份（含照片），以及带预算的恢复。
//!
//! 恢复有两种方式，都先预览再执行，不会静默覆盖现有记录：
//! - 追加（merge）：只加入编号不存在的藏品与借出单；编号已存在的记录跳过并计数，绝不改写现有记录；
//! - 替换（replace）：用备份整体替换当前台账，界面必须二次确认。
//! 无论哪种方式，结果都要重新通过完整校验（引用关系、日期、守恒）。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::model::*;

pub const FORMAT: &str = "toolforge.collection-ledger";
/// 备份文本上限
pub const MAX_BACKUP: usize = 16 * 1024 * 1024;
/// 单张照片（data URI）上限
pub const MAX_PHOTO: usize = 96 * 1024;
const PHOTO_PREFIX: &str = "data:image/jpeg;base64,";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub items: Vec<Item>,
    pub loans: Vec<Loan>,
    /// 藏品编号 → 照片 data URI
    #[serde(default)]
    pub photos: BTreeMap<String, String>,
}

/// 照片 data URI 校验：JPEG、大小在预算内
pub fn check_photo(data: &str) -> PluginResult<()> {
    if data.len() > MAX_PHOTO {
        return Err(error("photo_too_large"));
    }
    let body = data
        .strip_prefix(PHOTO_PREFIX)
        .ok_or_else(|| error("bad_photo"))?;
    let bytes = data_encoding::BASE64
        .decode(body.as_bytes())
        .map_err(|_| error("bad_photo"))?;
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(error("bad_photo"));
    }
    Ok(())
}

pub fn backup_json(
    ledger: &Ledger,
    photos: &BTreeMap<String, String>,
    today: &str,
) -> PluginResult<String> {
    validate(ledger)?;
    let mut kept = BTreeMap::new();
    for item in ledger.items.iter().filter(|i| i.has_photo) {
        if let Some(data) = photos.get(&item.id) {
            check_photo(data)?;
            kept.insert(item.id.clone(), data.clone());
        }
    }
    let backup = Backup {
        format: FORMAT.to_string(),
        version: SCHEMA,
        exported_at: today.to_string(),
        items: ledger.items.clone(),
        loans: ledger.loans.clone(),
        photos: kept,
    };
    serde_json::to_string_pretty(&backup).map_err(|_| error("bad_ledger"))
}

/// CSV 单元格：引号转义；以 = + - @ 开头的文本加前导单引号，防止表格软件当公式执行
fn cell(text: &str) -> String {
    let guarded = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_string()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

/// 藏品清单 CSV（UTF-8 BOM，表头由界面按语言传入）
pub fn list_csv(ledger: &Ledger, headers: &[String]) -> PluginResult<String> {
    validate(ledger)?;
    if headers.len() != 12 {
        return Err(error("bad_headers"));
    }
    let mut out = String::from("\u{feff}");
    out.push_str(
        &headers
            .iter()
            .map(|h| cell(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("\r\n");
    let mut items: Vec<&Item> = ledger.items.iter().collect();
    items.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.series.cmp(&b.series)));
    for item in items {
        let row = [
            cell(&item.name),
            cell(&item.series),
            cell(&item.edition),
            cell(&item.code),
            cell(&item.category),
            cell(&item.location),
            cell(&item.condition),
            item.owned.to_string(),
            ledger.available(item).to_string(),
            ledger.outstanding(&item.id).to_string(),
            if item.archived {
                "1".into()
            } else {
                "0".into()
            },
            cell(&item.notes),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Merge,
    Replace,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub items_added: usize,
    pub loans_added: usize,
    /// 编号已存在而跳过的记录（追加模式）
    pub items_skipped: usize,
    pub loans_skipped: usize,
    pub photos: usize,
    /// 替换模式下将被移除的现有记录
    pub items_removed: usize,
    pub loans_removed: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    /// 预览时为 None
    pub ledger: Option<Ledger>,
    /// 需要写入的照片（藏品编号 → data URI）
    pub photos: BTreeMap<String, String>,
    /// 需要删除的照片（替换模式下被移除的藏品）
    pub remove_photos: Vec<String>,
    pub report: Report,
}

pub fn restore(
    current: &Ledger,
    text: &str,
    mode: Mode,
    revision: u64,
    preview: bool,
) -> PluginResult<Restored> {
    validate(current)?;
    if revision != current.revision {
        return Err(error("stale"));
    }
    if text.len() > MAX_BACKUP {
        return Err(error("backup_too_large"));
    }
    let backup: Backup = serde_json::from_str(text).map_err(|_| error("bad_backup"))?;
    if backup.format != FORMAT || backup.version != SCHEMA {
        return Err(error("bad_backup"));
    }
    // 备份自身必须是完整、守恒的台账
    let incoming = Ledger {
        schema: SCHEMA,
        items: backup.items,
        loans: backup.loans,
        ..Ledger::default()
    };
    validate(&incoming)?;
    if backup.photos.len() > incoming.items.len() {
        return Err(error("bad_backup"));
    }
    for (id, data) in &backup.photos {
        incoming.item(id).map_err(|_| error("bad_backup"))?;
        check_photo(data)?;
    }

    let mut report = Report::default();
    let mut next = current.clone();
    let mut remove_photos = Vec::new();
    let added_items: BTreeSet<String>;
    match mode {
        Mode::Replace => {
            report.items_removed = current.items.len();
            report.loans_removed = current.loans.len();
            remove_photos = current
                .items
                .iter()
                .filter(|i| i.has_photo)
                .map(|i| i.id.clone())
                .collect();
            next.items = incoming.items.clone();
            next.loans = incoming.loans.clone();
            report.items_added = next.items.len();
            report.loans_added = next.loans.len();
            added_items = next.items.iter().map(|i| i.id.clone()).collect();
        }
        Mode::Merge => {
            let existing: BTreeSet<String> = current
                .items
                .iter()
                .map(|i| i.id.clone())
                .chain(current.loans.iter().map(|l| l.id.clone()))
                .collect();
            let mut added = BTreeSet::new();
            for item in incoming.items {
                if existing.contains(&item.id) {
                    report.items_skipped += 1;
                } else {
                    added.insert(item.id.clone());
                    next.items.push(item);
                    report.items_added += 1;
                }
            }
            for loan in incoming.loans {
                if existing.contains(&loan.id) {
                    report.loans_skipped += 1;
                } else if !added.contains(&loan.item_id) {
                    // 借出单只跟随新加入的藏品；不给现有藏品追加外来借出，避免改动其数量
                    report.loans_skipped += 1;
                } else {
                    next.loans.push(loan);
                    report.loans_added += 1;
                }
            }
            added_items = added;
        }
    }
    // 照片：只写入新加入的藏品；没有照片数据的藏品去掉照片标记
    let mut photos = BTreeMap::new();
    for item in next
        .items
        .iter_mut()
        .filter(|i| added_items.contains(&i.id))
    {
        match backup.photos.get(&item.id) {
            Some(data) if item.has_photo => {
                photos.insert(item.id.clone(), data.clone());
            }
            _ => item.has_photo = false,
        }
    }
    report.photos = photos.len();
    if next.items.len() > MAX_ITEMS || next.loans.len() > MAX_LOANS {
        return Err(error("too_many"));
    }
    validate(&next)?;
    next.undo = None;
    next.revision = current.revision + 1;
    check_size(&next)?;
    Ok(Restored {
        ledger: (!preview).then_some(next),
        photos: if preview { BTreeMap::new() } else { photos },
        remove_photos: if preview { Vec::new() } else { remove_photos },
        report,
    })
}

#[cfg(test)]
#[path = "backup_test.rs"]
mod tests;
