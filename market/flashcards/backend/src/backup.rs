//! 备份与恢复：导出含进度的卡片集合（JSON），恢复前预览冲突（同 id 但内容不同），
//! 应用时按 overwrite / skip 处理冲突，replace 整体替换。全部校验在 Rust 侧完成。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::schedule::{MAX_CARDS, MAX_DECKS, MAX_TAGS, MAX_TEXT};

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 备份里的单张卡（含学习进度与历史）
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupCard {
    pub id: String,
    #[serde(default)]
    pub deck: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub stage: i64,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub version: i64,
    pub exported_at: String,
    pub cards: Vec<BackupCard>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub cards: Vec<BackupCard>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOut {
    pub json: String,
    pub count: i64,
}

/// 导出：校验规模后序列化为带版本号的 JSON
pub fn export(args: ExportArgs) -> PluginResult<ExportOut> {
    if args.cards.len() > MAX_CARDS {
        return Err(errp("fcs.too_many_cards", "max", MAX_CARDS));
    }
    validate_cards(&args.cards)?;
    let backup = Backup {
        version: 1,
        exported_at: String::new(),
        cards: args.cards,
    };
    let json = serde_json::to_string(&backup).map_err(|_| PluginError::new("fcs.serialize"))?;
    Ok(ExportOut {
        count: backup.cards.len() as i64,
        json,
    })
}

fn validate_cards(cards: &[BackupCard]) -> PluginResult<()> {
    let mut decks = std::collections::BTreeSet::new();
    for card in cards {
        if card.id.is_empty() {
            return Err(PluginError::new("fcs.empty_id"));
        }
        if !card.deck.is_empty() {
            decks.insert(card.deck.clone());
        }
        if card.question.chars().count() > MAX_TEXT || card.answer.chars().count() > MAX_TEXT {
            return Err(errp("fcs.text_too_long", "max", MAX_TEXT));
        }
        if card.tags.len() > MAX_TAGS {
            return Err(errp("fcs.too_many_tags", "max", MAX_TAGS));
        }
    }
    if decks.len() > MAX_DECKS {
        return Err(errp("fcs.too_many_decks", "max", MAX_DECKS));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewArgs {
    pub json: String,
    #[serde(default)]
    pub current: Vec<BackupCard>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: String,
    pub question: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOut {
    pub incoming: i64,
    /// 同 id 且内容不同的卡
    pub conflicts: Vec<Conflict>,
    /// 全新卡数
    pub fresh: i64,
}

/// 恢复预览：解析备份并列出冲突（同 id、问答不同）
pub fn restore_preview(args: PreviewArgs) -> PluginResult<PreviewOut> {
    let backup = parse_backup(&args.json)?;
    let current_by_id: std::collections::BTreeMap<&str, &BackupCard> = args
        .current
        .iter()
        .map(|card| (card.id.as_str(), card))
        .collect();
    let mut conflicts = Vec::new();
    let mut fresh = 0i64;
    for card in &backup.cards {
        match current_by_id.get(card.id.as_str()) {
            Some(existing) => {
                if existing.question != card.question
                    || existing.answer != card.answer
                    || existing.deck != card.deck
                {
                    conflicts.push(Conflict {
                        id: card.id.clone(),
                        question: card.question.clone(),
                    });
                }
            }
            None => fresh += 1,
        }
    }
    Ok(PreviewOut {
        incoming: backup.cards.len() as i64,
        conflicts,
        fresh,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyArgs {
    pub json: String,
    #[serde(default)]
    pub current: Vec<BackupCard>,
    /// overwrite=备份覆盖同 id；skip=保留现状；replace=整体替换
    pub mode: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOut {
    pub cards: Vec<BackupCard>,
    pub overwritten: i64,
    pub skipped: i64,
}

/// 应用恢复：按模式合并，输出完整卡片集合（前端整体保存）
pub fn restore_apply(args: ApplyArgs) -> PluginResult<ApplyOut> {
    let backup = parse_backup(&args.json)?;
    validate_cards(&backup.cards)?;
    let mut overwritten = 0i64;
    let mut skipped = 0i64;
    let cards = if args.mode == "replace" {
        backup.cards
    } else {
        if args.mode != "overwrite" && args.mode != "skip" {
            return Err(errp("fcs.bad_mode", "mode", args.mode.as_str()));
        }
        let incoming_by_id: std::collections::BTreeMap<&str, BackupCard> = backup
            .cards
            .iter()
            .map(|card| (card.id.as_str(), card.clone()))
            .collect();
        let mut merged: Vec<BackupCard> = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for card in &args.current {
            if let Some(incoming) = incoming_by_id.get(card.id.as_str()) {
                if args.mode == "overwrite" {
                    overwritten += 1;
                    merged.push(incoming.clone());
                } else {
                    skipped += 1;
                    merged.push(card.clone());
                }
            } else {
                merged.push(card.clone());
            }
            seen.insert(card.id.clone());
        }
        for card in &backup.cards {
            if !seen.contains(&card.id) {
                merged.push(card.clone());
            }
        }
        if merged.len() > MAX_CARDS {
            return Err(errp("fcs.too_many_cards", "max", MAX_CARDS));
        }
        merged
    };
    Ok(ApplyOut {
        cards,
        overwritten,
        skipped,
    })
}

fn parse_backup(json: &str) -> PluginResult<Backup> {
    let backup: Backup =
        serde_json::from_str(json).map_err(|_| PluginError::new("fcs.bad_backup"))?;
    if backup.version != 1 {
        return Err(errp("fcs.bad_version", "version", backup.version));
    }
    if backup.cards.len() > MAX_CARDS {
        return Err(errp("fcs.too_many_cards", "max", MAX_CARDS));
    }
    Ok(backup)
}

#[cfg(test)]
#[path = "backup_test.rs"]
mod tests;
