//! 作品结构与帧操作。
//!
//! - 帧内容不可变：每次画笔、橡皮或导入都生成新的帧编号，旧编号的内容保持原样。
//!   因此复制帧天然不共享可变内容，撤销/重做只需要回到旧的帧编号列表。
//! - `pool` 记录本作品在存储中可能存在的全部帧编号（含撤销历史仍引用的），
//!   清理时据此删除不再被引用的帧，避免存储无限增长。
//! - 作品至少保留一帧；帧数、单帧与整体容量都有上限（见 frame.rs）。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::frame::{MAX_FRAMES, MAX_PROJECT_TEXT, check_size, error, parse_color};

/// 每帧停留时长范围与步长（毫秒）；10 毫秒对应 GIF 的最小时间单位
pub const MIN_DELAY: u32 = 20;
pub const MAX_DELAY: u32 = 2000;
pub const DELAY_STEP: u32 = 10;
/// 作品名称长度上限（字符）
const MAX_NAME: usize = 60;
/// pool 中编号数量上限（当前帧 + 撤销历史引用的帧）
const MAX_POOL: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub delay_ms: u32,
    /// GIF 导出底色 `#rrggbb`
    pub background: String,
    /// 当前帧顺序（帧编号）
    pub frames: Vec<String>,
    /// 存储中可能存在的帧编号
    pub pool: Vec<String>,
    /// 下一个帧编号的序号
    pub next_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Op {
    /// 在 index 之后插入空白帧
    Add {
        index: usize,
    },
    /// 复制 index 处的帧，插在它后面
    Duplicate {
        index: usize,
    },
    Delete {
        index: usize,
    },
    /// 把 from 处的帧移到 to（移动后的位置）
    Move {
        from: usize,
        to: usize,
    },
    /// index 处的帧内容被替换（笔画/导入后的新内容），`size` 为新内容的存档长度
    Replace {
        index: usize,
        size: u64,
    },
    /// 统一的每帧停留时长
    Delay {
        ms: u32,
    },
    Background {
        color: String,
    },
    Rename {
        name: String,
    },
    /// 撤销/重做：直接换成给定的帧顺序（只能引用 pool 中已有的帧）
    Restore {
        frames: Vec<String>,
    },
}

/// 操作结果：新作品，以及需要写入内容的新帧（`source` 为复制来源）
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub project: Project,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<Created>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

fn clean_name(name: &str) -> PluginResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(error("bad_name"));
    }
    Ok(name.to_string())
}

/// 停留时长按 10 毫秒取整并限制在范围内
pub fn normalize_delay(ms: u32) -> u32 {
    let rounded = (ms + DELAY_STEP / 2) / DELAY_STEP * DELAY_STEP;
    rounded.clamp(MIN_DELAY, MAX_DELAY)
}

/// 新建作品：一帧空白
pub fn create(id: &str, name: &str, width: u32, height: u32) -> PluginResult<Project> {
    check_size(width, height)?;
    if id.is_empty() || id.len() > 40 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(error("bad_project"));
    }
    Ok(Project {
        id: id.to_string(),
        name: clean_name(name)?,
        width,
        height,
        delay_ms: 120,
        background: "#ffffff".to_string(),
        frames: vec!["f1".to_string()],
        pool: vec!["f1".to_string()],
        next_id: 2,
    })
}

/// 校验从存储读回或由界面传来的作品
pub fn validate(p: &Project) -> PluginResult<()> {
    check_size(p.width, p.height)?;
    parse_color(&p.background)?;
    clean_name(&p.name)?;
    if p.frames.is_empty() || p.frames.len() > MAX_FRAMES {
        return Err(error("bad_project"));
    }
    if !(MIN_DELAY..=MAX_DELAY).contains(&p.delay_ms) || p.pool.len() > MAX_POOL {
        return Err(error("bad_project"));
    }
    let pool: BTreeSet<&String> = p.pool.iter().collect();
    let unique: BTreeSet<&String> = p.frames.iter().collect();
    if unique.len() != p.frames.len() || !p.frames.iter().all(|f| pool.contains(f)) {
        return Err(error("bad_project"));
    }
    Ok(())
}

fn index_ok(p: &Project, index: usize) -> PluginResult<()> {
    if index < p.frames.len() {
        Ok(())
    } else {
        Err(error("bad_index"))
    }
}

fn fresh(p: &mut Project) -> PluginResult<String> {
    if p.pool.len() >= MAX_POOL {
        return Err(error("history_full"));
    }
    let id = format!("f{}", p.next_id);
    p.next_id += 1;
    p.pool.push(id.clone());
    Ok(id)
}

/// 当前帧占用的存档容量；`sizes` 由界面给出（各帧存档文本长度，空白帧为 0）
fn used(frames: &[String], sizes: &BTreeMap<String, u64>) -> u64 {
    frames
        .iter()
        .map(|f| sizes.get(f).copied().unwrap_or(0))
        .sum()
}

/// 应用一次操作
pub fn apply(mut p: Project, op: Op, sizes: &BTreeMap<String, u64>) -> PluginResult<Edit> {
    validate(&p)?;
    let mut created = None;
    let mut extra = 0u64;
    match op {
        Op::Add { index } => {
            index_ok(&p, index)?;
            if p.frames.len() >= MAX_FRAMES {
                return Err(error("too_many_frames").with("max", MAX_FRAMES));
            }
            let id = fresh(&mut p)?;
            p.frames.insert(index + 1, id.clone());
            created = Some(Created { id, source: None });
        }
        Op::Duplicate { index } => {
            index_ok(&p, index)?;
            if p.frames.len() >= MAX_FRAMES {
                return Err(error("too_many_frames").with("max", MAX_FRAMES));
            }
            let source = p.frames[index].clone();
            let id = fresh(&mut p)?;
            extra = sizes.get(&source).copied().unwrap_or(0);
            p.frames.insert(index + 1, id.clone());
            created = Some(Created {
                id,
                source: Some(source),
            });
        }
        Op::Delete { index } => {
            index_ok(&p, index)?;
            if p.frames.len() == 1 {
                return Err(error("last_frame"));
            }
            p.frames.remove(index);
        }
        Op::Move { from, to } => {
            index_ok(&p, from)?;
            index_ok(&p, to)?;
            let id = p.frames.remove(from);
            p.frames.insert(to, id);
        }
        Op::Replace { index, size } => {
            index_ok(&p, index)?;
            let id = fresh(&mut p)?;
            p.frames[index] = id.clone();
            extra = size;
            created = Some(Created { id, source: None });
        }
        Op::Delay { ms } => p.delay_ms = normalize_delay(ms),
        Op::Background { color } => {
            parse_color(&color)?;
            p.background = color.to_ascii_lowercase();
        }
        Op::Rename { name } => p.name = clean_name(&name)?,
        Op::Restore { frames } => {
            let pool: BTreeSet<&String> = p.pool.iter().collect();
            if frames.is_empty() || !frames.iter().all(|f| pool.contains(f)) {
                return Err(error("bad_project"));
            }
            p.frames = frames;
        }
    }
    // 新帧的内容尚未写入 sizes，单独计入
    let total = used(&p.frames, sizes) + extra;
    if total > MAX_PROJECT_TEXT {
        return Err(error("project_full").with("mb", MAX_PROJECT_TEXT / (1024 * 1024)));
    }
    validate(&p)?;
    Ok(Edit {
        project: p,
        created,
    })
}

/// 清理：只保留当前帧与 `keep`（撤销历史仍引用的帧），返回新作品与可删除的帧编号
pub fn collect(mut p: Project, keep: &[String]) -> PluginResult<(Project, Vec<String>)> {
    validate(&p)?;
    let live: BTreeSet<&String> = p.frames.iter().chain(keep).collect();
    let (stay, gone): (Vec<String>, Vec<String>) =
        p.pool.iter().cloned().partition(|id| live.contains(id));
    p.pool = stay;
    Ok((p, gone))
}

#[cfg(test)]
#[path = "project_test.rs"]
mod tests;
