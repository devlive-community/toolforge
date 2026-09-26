//! 查找重复：按大小分组 → 比较开头 64 KB 的哈希 → 比较完整 BLAKE3 哈希，多线程读取。

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use serde::Serialize;
use tf_plugin_api::{PluginResult, cancelled};

use crate::scan::FileInfo;

/// 先比较的开头字节数；不超过它的文件直接得到完整哈希
pub const HEAD: u64 = 64 * 1024;
const BUFFER: usize = 1024 * 1024;
const MAX_WORKERS: usize = 8;

/// 计算哈希；limit 为 None 时读取整个文件
pub fn hash_file(
    path: &Path,
    limit: Option<u64>,
    cancelled_flag: &dyn Fn() -> bool,
) -> std::io::Result<Option<String>> {
    let file = File::open(path)?;
    let mut reader: Box<dyn Read> = match limit {
        Some(limit) => Box::new(file.take(limit)),
        None => Box::new(file),
    };
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER];
    loop {
        if cancelled_flag() {
            return Ok(None);
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some(hasher.finalize().to_hex().to_string()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    pub path: String,
    pub modified: i64,
    pub root: usize,
    /// 与本组其他文件是同一文件的硬链接
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub hash: String,
    pub size: u64,
    pub files: Vec<Copy>,
    /// 删除多余副本可释放的字节数
    pub wasted: u64,
}

pub struct Progress<'a> {
    pub stage: &'a dyn Fn(&str),
    pub bytes: &'a dyn Fn(u64, u64),
    pub cancelled: &'a (dyn Fn() -> bool + Sync),
}

/// 把硬链接合并为一个代表文件
fn merge_links(files: Vec<FileInfo>) -> Vec<(FileInfo, Vec<String>)> {
    let mut by_inode: HashMap<(u64, u64), usize> = HashMap::new();
    let mut out: Vec<(FileInfo, Vec<String>)> = Vec::new();
    for file in files {
        if let Some(key) = file.inode {
            if let Some(&index) = by_inode.get(&key) {
                out[index].1.push(file.path.to_string_lossy().into_owned());
                continue;
            }
            by_inode.insert(key, out.len());
        }
        out.push((file, Vec::new()));
    }
    out
}

/// 并行计算一批文件的哈希；读取失败的文件被忽略
fn hash_all(
    files: &[&Entry],
    limit: Option<u64>,
    total: u64,
    done: &AtomicU64,
    progress: &Progress,
) -> PluginResult<Vec<Option<String>>> {
    let results: Mutex<Vec<Option<String>>> = Mutex::new(vec![None; files.len()]);
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(MAX_WORKERS)
        .min(files.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= files.len() || (progress.cancelled)() {
                        break;
                    }
                    let (file, _) = files[index];
                    let hash = hash_file(&file.path, limit, progress.cancelled)
                        .ok()
                        .flatten();
                    if let Ok(mut results) = results.lock() {
                        results[index] = hash;
                    }
                    let read = limit.map_or(file.size, |l| l.min(file.size));
                    done.fetch_add(read, Ordering::Relaxed);
                }
            });
        }
        // 主线程定期汇报进度
        while next.load(Ordering::Relaxed) < files.len() && !(progress.cancelled)() {
            (progress.bytes)(done.load(Ordering::Relaxed), total);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
    if (progress.cancelled)() {
        return Err(cancelled());
    }
    (progress.bytes)(done.load(Ordering::Relaxed), total);
    Ok(results.into_inner().unwrap_or_default())
}

type Entry = (FileInfo, Vec<String>);

/// 按键把文件分组，只保留两个及以上的组
fn regroup<'a, K: std::hash::Hash + Eq>(
    items: impl Iterator<Item = (K, &'a Entry)>,
) -> Vec<(K, Vec<&'a Entry>)> {
    let mut groups: HashMap<K, Vec<&Entry>> = HashMap::new();
    for (key, item) in items {
        groups.entry(key).or_default().push(item);
    }
    groups.into_iter().filter(|(_, g)| g.len() > 1).collect()
}

pub fn find(files: Vec<FileInfo>, progress: &Progress) -> PluginResult<Vec<Group>> {
    let files = merge_links(files);
    // 第一步：大小相同
    let candidates: Vec<&Entry> = regroup(files.iter().map(|f| (f.0.size, f)))
        .into_iter()
        .flat_map(|(_, group)| group)
        .collect();

    // 第二步：开头 64 KB 相同；不超过 64 KB 的文件开头哈希就是完整哈希
    (progress.stage)("dup.head");
    let head_total: u64 = candidates.iter().map(|f| f.0.size.min(HEAD)).sum();
    let heads = hash_all(
        &candidates,
        Some(HEAD),
        head_total,
        &AtomicU64::new(0),
        progress,
    )?;
    let mut groups: Vec<Group> = Vec::new();
    let mut large: Vec<&Entry> = Vec::new();
    for ((size, hash), group) in regroup(
        candidates
            .iter()
            .zip(&heads)
            .filter_map(|(f, h)| h.as_ref().map(|h| ((f.0.size, h.clone()), *f))),
    ) {
        if size <= HEAD {
            groups.push(make_group(hash, &group));
        } else {
            large.extend(group);
        }
    }

    // 第三步：完整哈希
    (progress.stage)("dup.full");
    let full_total: u64 = large.iter().map(|f| f.0.size).sum();
    let fulls = hash_all(&large, None, full_total, &AtomicU64::new(0), progress)?;
    for (hash, group) in regroup(
        large
            .iter()
            .zip(&fulls)
            .filter_map(|(f, h)| h.as_ref().map(|h| (h.clone(), *f))),
    ) {
        groups.push(make_group(hash, &group));
    }
    groups.sort_by(|a, b| {
        b.wasted
            .cmp(&a.wasted)
            .then_with(|| a.files[0].path.cmp(&b.files[0].path))
    });
    Ok(groups)
}

fn make_group(hash: String, members: &[&Entry]) -> Group {
    let mut files: Vec<Copy> = members
        .iter()
        .map(|(file, links)| Copy {
            path: file.path.to_string_lossy().into_owned(),
            modified: file.modified,
            root: file.root,
            links: links.clone(),
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let size = members[0].0.size;
    Group {
        hash,
        size,
        wasted: size * (files.len() as u64 - 1),
        files,
    }
}

#[cfg(test)]
#[path = "find_test.rs"]
mod tests;
