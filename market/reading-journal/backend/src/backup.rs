//! 导出与恢复：作品与经历的 CSV、完整 JSON 备份（含用户自带封面），以及带预算的恢复。
//!
//! 恢复先预览再执行，不会静默覆盖：
//! - 追加（merge）：只加入编号不存在的作品与经历；编号已存在的跳过并计数，
//!   所以同一份备份恢复两次也不会把经历算两遍；
//! - 替换（replace）：用备份整体替换，界面必须二次确认。
//!
//! 无论哪种方式，结果都要重新通过完整校验（引用关系、日期顺序、进度上限）。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::model::*;

pub const FORMAT: &str = "toolforge.reading-journal";
pub const MAX_BACKUP: usize = 16 * 1024 * 1024;
pub const MAX_COVER: usize = 96 * 1024;
const COVER_PREFIX: &str = "data:image/jpeg;base64,";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub works: Vec<Work>,
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub covers: BTreeMap<String, String>,
}

pub fn check_cover(data: &str) -> PluginResult<()> {
    if data.len() > MAX_COVER {
        return Err(error("cover_too_large"));
    }
    let body = data
        .strip_prefix(COVER_PREFIX)
        .ok_or_else(|| error("bad_cover"))?;
    let bytes = data_encoding::BASE64
        .decode(body.as_bytes())
        .map_err(|_| error("bad_cover"))?;
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(error("bad_cover"));
    }
    Ok(())
}

pub fn backup_json(
    journal: &Journal,
    covers: &BTreeMap<String, String>,
    today: &str,
) -> PluginResult<String> {
    validate(journal)?;
    let mut kept = BTreeMap::new();
    for work in journal.works.iter().filter(|w| w.has_cover) {
        if let Some(data) = covers.get(&work.id) {
            check_cover(data)?;
            kept.insert(work.id.clone(), data.clone());
        }
    }
    let backup = Backup {
        format: FORMAT.into(),
        version: SCHEMA,
        exported_at: today.into(),
        works: journal.works.clone(),
        sessions: journal.sessions.clone(),
        covers: kept,
    };
    serde_json::to_string_pretty(&backup).map_err(|_| error("bad_journal"))
}

/// 解析备份文本并校验为完整手账（命令行年度回顾与恢复共用）
pub fn parse_backup(text: &str) -> PluginResult<(Journal, BTreeMap<String, String>)> {
    if text.len() > MAX_BACKUP {
        return Err(error("backup_too_large"));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let backup: Backup = serde_json::from_str(text).map_err(|_| error("bad_backup"))?;
    if backup.format != FORMAT || backup.version != SCHEMA {
        return Err(error("bad_backup"));
    }
    let journal = Journal {
        schema: SCHEMA,
        works: backup.works,
        sessions: backup.sessions,
        ..Journal::default()
    };
    validate(&journal)?;
    if backup.covers.len() > journal.works.len() {
        return Err(error("bad_backup"));
    }
    for (id, data) in &backup.covers {
        journal.work(id).map_err(|_| error("bad_backup"))?;
        check_cover(data)?;
    }
    Ok((journal, backup.covers))
}

fn cell(text: &str) -> String {
    let guarded = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_string()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

/// 每次经历一行（没有经历的作品也占一行），表头与类型/状态名称由界面按语言传入
pub fn list_csv(
    journal: &Journal,
    headers: &[String],
    labels: &BTreeMap<String, String>,
) -> PluginResult<String> {
    validate(journal)?;
    if headers.len() != 14 {
        return Err(error("bad_headers"));
    }
    let label = |key: &str| labels.get(key).cloned().unwrap_or_else(|| key.to_string());
    let mut out = String::from("\u{feff}");
    out.push_str(
        &headers
            .iter()
            .map(|h| cell(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("\r\n");
    let mut works: Vec<&Work> = journal.works.iter().collect();
    works.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.cmp(&b.id)));
    for work in works {
        let kind = label(match work.kind {
            Kind::Book => "book",
            Kind::Film => "film",
        });
        let status = label(match work.status {
            Status::Want => "want",
            Status::Doing => "doing",
            Status::Done => "done",
            Status::Paused => "paused",
        });
        let head = [
            cell(&kind),
            cell(&work.title),
            cell(&work.creator),
            work.year.map(|y| y.to_string()).unwrap_or_default(),
            cell(&work.genre),
            cell(&work.tags.join(" / ")),
            cell(&status),
            work.total.map(|t| t.to_string()).unwrap_or_default(),
        ];
        let mut sessions: Vec<&Session> = journal.sessions_of(&work.id).collect();
        sessions.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
        if sessions.is_empty() {
            out.push_str(&head.join(","));
            out.push_str(",,,,,,\r\n");
        }
        for s in sessions {
            let tail = [
                s.start.clone().unwrap_or_default(),
                s.finish.clone().unwrap_or_default(),
                s.progress.to_string(),
                if s.completed { "1".into() } else { "0".into() },
                s.rating.map(|r| r.to_string()).unwrap_or_default(),
                cell(&s.review),
            ];
            out.push_str(&head.join(","));
            out.push(',');
            out.push_str(&tail.join(","));
            out.push_str("\r\n");
        }
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
    pub works_added: usize,
    pub sessions_added: usize,
    pub works_skipped: usize,
    pub sessions_skipped: usize,
    pub covers: usize,
    pub works_removed: usize,
    pub sessions_removed: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    pub journal: Option<Journal>,
    pub covers: BTreeMap<String, String>,
    pub remove_covers: Vec<String>,
    pub report: Report,
}

pub fn restore(
    current: &Journal,
    text: &str,
    mode: Mode,
    revision: u64,
    preview: bool,
) -> PluginResult<Restored> {
    validate(current)?;
    if revision != current.revision {
        return Err(error("stale"));
    }
    let (incoming, covers) = parse_backup(text)?;
    let mut report = Report::default();
    let mut next = current.clone();
    let mut remove_covers = Vec::new();
    let added: BTreeSet<String>;
    match mode {
        Mode::Replace => {
            report.works_removed = current.works.len();
            report.sessions_removed = current.sessions.len();
            remove_covers = current
                .works
                .iter()
                .filter(|w| w.has_cover)
                .map(|w| w.id.clone())
                .collect();
            next.works = incoming.works.clone();
            next.sessions = incoming.sessions.clone();
            report.works_added = next.works.len();
            report.sessions_added = next.sessions.len();
            added = next.works.iter().map(|w| w.id.clone()).collect();
        }
        Mode::Merge => {
            let existing: BTreeSet<String> = current
                .works
                .iter()
                .map(|w| w.id.clone())
                .chain(current.sessions.iter().map(|s| s.id.clone()))
                .collect();
            let mut new_works = BTreeSet::new();
            for work in incoming.works {
                if existing.contains(&work.id) {
                    report.works_skipped += 1;
                } else {
                    new_works.insert(work.id.clone());
                    next.works.push(work);
                    report.works_added += 1;
                }
            }
            for session in incoming.sessions {
                // 编号已存在说明是同一次经历，跳过，避免恢复两次后完成次数翻倍
                if existing.contains(&session.id) {
                    report.sessions_skipped += 1;
                } else {
                    next.sessions.push(session);
                    report.sessions_added += 1;
                }
            }
            added = new_works;
        }
    }
    let mut kept = BTreeMap::new();
    for work in next.works.iter_mut().filter(|w| added.contains(&w.id)) {
        match covers.get(&work.id) {
            Some(data) if work.has_cover => {
                kept.insert(work.id.clone(), data.clone());
            }
            _ => work.has_cover = false,
        }
    }
    report.covers = kept.len();
    if next.works.len() > MAX_WORKS || next.sessions.len() > MAX_SESSIONS {
        return Err(error("too_many"));
    }
    validate(&next)?;
    next.undo = None;
    next.revision = current.revision + 1;
    check_size(&next)?;
    Ok(Restored {
        journal: (!preview).then_some(next),
        covers: if preview { BTreeMap::new() } else { kept },
        remove_covers: if preview { Vec::new() } else { remove_covers },
        report,
    })
}

#[cfg(test)]
#[path = "backup_test.rs"]
mod tests;
