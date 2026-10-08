//! 合并两侧的遍历结果，判断每个条目的状态，文件夹的状态由其内容汇总而来。

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::scan::{Entry, Kind};

/// 修改时间误差（FAT 文件系统与压缩包只精确到 2 秒）
const TIME_TOLERANCE: i64 = 2000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    /// 大小与修改时间
    #[default]
    Quick,
    /// 大小相同时比较内容
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Same,
    Changed,
    LeftOnly,
    RightOnly,
    /// 一侧是文件，另一侧是文件夹
    Mismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub dir: bool,
    pub size: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: String,
    pub name: String,
    pub depth: usize,
    /// 两侧中至少一侧是文件夹
    pub dir: bool,
    pub status: Status,
    pub left: Option<Info>,
    pub right: Option<Info>,
    /// 内容不同的文件中修改时间较新的一侧
    pub newer: Option<Side>,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub same: usize,
    pub changed: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub mismatch: usize,
}

fn info(entry: &Entry) -> Info {
    Info {
        dir: entry.kind == Kind::Dir,
        size: entry.size,
        modified: entry.modified,
    }
}

pub fn hash(path: &Path) -> Option<blake3::Hash> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0; 256 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize())
}

/// 两侧都是文件时的状态；`same_content` 为内容比较结果（快速模式下为 None）
fn file_status(l: &Entry, r: &Entry, same_content: Option<bool>) -> Status {
    if l.size != r.size {
        return Status::Changed;
    }
    match same_content {
        Some(true) => Status::Same,
        Some(false) => Status::Changed,
        None if (l.modified - r.modified).abs() <= TIME_TOLERANCE => Status::Same,
        None => Status::Changed,
    }
}

/// 内容模式下需要比较内容的文件（两侧都是文件且大小相同）
pub fn candidates(
    left: &BTreeMap<String, Entry>,
    right: &BTreeMap<String, Entry>,
) -> Vec<(String, u64)> {
    left.iter()
        .filter_map(|(path, l)| {
            let r = right.get(path)?;
            (l.kind == Kind::File && r.kind == Kind::File && l.size == r.size)
                .then(|| (path.clone(), l.size))
        })
        .collect()
}

/// 并行比较内容，返回内容相同的路径
pub fn same_contents(
    left_root: &Path,
    right_root: &Path,
    paths: &[(String, u64)],
    on_done: &(dyn Fn(u64) + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> BTreeSet<String> {
    paths
        .par_iter()
        .filter_map(|(path, size)| {
            if cancelled() {
                return None;
            }
            let rel = Path::new(path);
            let same = *size == 0 || {
                let l = hash(&left_root.join(rel));
                l.is_some() && l == hash(&right_root.join(rel))
            };
            on_done(*size);
            same.then(|| path.clone())
        })
        .collect()
}

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(p, _)| p)
}

pub fn merge(
    left: &BTreeMap<String, Entry>,
    right: &BTreeMap<String, Entry>,
    method: Method,
    same: &BTreeSet<String>,
) -> (Vec<Item>, Summary) {
    let paths: BTreeSet<&String> = left.keys().chain(right.keys()).collect();
    let mut items: BTreeMap<&str, Item> = BTreeMap::new();
    let mut summary = Summary::default();
    for path in &paths {
        let (l, r) = (left.get(*path), right.get(*path));
        let status = match (l, r) {
            (Some(_), None) => Status::LeftOnly,
            (None, Some(_)) => Status::RightOnly,
            (Some(l), Some(r)) if l.kind != r.kind => Status::Mismatch,
            (Some(l), Some(_)) if l.kind == Kind::Dir => Status::Same,
            (Some(l), Some(r)) => file_status(
                l,
                r,
                (method == Method::Content && l.size == r.size).then(|| same.contains(*path)),
            ),
            (None, None) => unreachable!(),
        };
        let dir = l.is_some_and(|e| e.kind == Kind::Dir) || r.is_some_and(|e| e.kind == Kind::Dir);
        if !dir {
            match status {
                Status::Same => summary.same += 1,
                Status::Changed => summary.changed += 1,
                Status::LeftOnly => summary.left_only += 1,
                Status::RightOnly => summary.right_only += 1,
                Status::Mismatch => summary.mismatch += 1,
            }
        } else if status == Status::Mismatch {
            summary.mismatch += 1;
        }
        let newer = match (l, r, status) {
            (Some(l), Some(r), Status::Changed) if l.modified != r.modified => {
                Some(if l.modified > r.modified {
                    Side::Left
                } else {
                    Side::Right
                })
            }
            _ => None,
        };
        items.insert(
            path.as_str(),
            Item {
                path: (*path).clone(),
                name: path.rsplit('/').next().unwrap_or(path).to_owned(),
                depth: path.matches('/').count(),
                dir,
                status,
                left: l.map(info),
                right: r.map(info),
                newer,
            },
        );
    }
    // 两侧都有的文件夹：内容有差异时标记为已变化（从最深处往上汇总）
    let mut keys: Vec<&str> = items.keys().copied().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.matches('/').count()));
    for key in keys {
        let status = items[key].status;
        if status == Status::Same {
            continue;
        }
        let mut current = parent(key);
        while let Some(p) = current {
            match items.get_mut(p) {
                Some(item) if item.status == Status::Same => item.status = Status::Changed,
                _ => break,
            }
            current = parent(p);
        }
    }
    let mut items: Vec<Item> = items.into_values().collect();
    items.sort_by_cached_key(tree_key);
    (items, summary)
}

/// 树形顺序：逐级比较名称，同一级中文件夹在前
fn tree_key(item: &Item) -> Vec<(bool, String)> {
    let parts: Vec<&str> = item.path.split('/').collect();
    let last = parts.len() - 1;
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| (i == last && !item.dir, part.to_lowercase()))
        .collect()
}

#[cfg(test)]
#[path = "compare_test.rs"]
mod tests;
