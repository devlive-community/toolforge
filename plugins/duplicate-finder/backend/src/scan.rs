//! 遍历文件夹收集候选文件：跳过符号链接、隐藏文件与排除的文件夹，记录大小、修改时间与 inode。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult, cancelled};

fn default_exclude() -> Vec<String> {
    [
        "node_modules",
        ".git",
        "$RECYCLE.BIN",
        "System Volume Information",
    ]
    .map(String::from)
    .to_vec()
}

fn one() -> u64 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub roots: Vec<String>,
    /// 小于该字节数的文件不参与比较（默认跳过空文件）
    #[serde(default = "one")]
    pub min_size: u64,
    #[serde(default)]
    pub include_hidden: bool,
    /// 按名称排除的文件夹
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
    /// 只比较这些扩展名（为空表示全部）
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    /// 修改时间（毫秒）
    pub modified: i64,
    /// (设备, inode)；硬链接指向同一份数据，不算重复
    pub inode: Option<(u64, u64)>,
    /// 所在根目录的序号，用于“保留第一个文件夹中的文件”
    pub root: usize,
}

#[cfg(unix)]
fn inode(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn inode(_: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

fn hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// 去掉嵌套的根目录（同时选了 A 和 A/B 时只扫描 A）
fn normalize_roots(roots: &[String]) -> PluginResult<Vec<PathBuf>> {
    let mut resolved = Vec::new();
    for root in roots {
        let path = Path::new(root)
            .canonicalize()
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| PluginError::new("dup.not_a_folder").with("path", root.as_str()))?;
        resolved.push(path);
    }
    let mut kept: Vec<PathBuf> = Vec::new();
    for path in &resolved {
        let nested = resolved
            .iter()
            .any(|other| other != path && path.starts_with(other));
        if !nested && !kept.contains(path) {
            kept.push(path.clone());
        }
    }
    Ok(kept)
}

pub fn scan(
    options: &Options,
    mut progress: impl FnMut(usize),
    is_cancelled: impl Fn() -> bool,
) -> PluginResult<Vec<FileInfo>> {
    if options.roots.is_empty() {
        return Err(PluginError::new("dup.no_folders"));
    }
    let roots = normalize_roots(&options.roots)?;
    let extensions: Vec<String> = options
        .extensions
        .iter()
        .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|e| !e.is_empty())
        .collect();
    let mut files = Vec::new();
    for (index, root) in roots.iter().enumerate() {
        let walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                if entry.depth() == 0 {
                    return true;
                }
                let name = entry.file_name().to_string_lossy();
                if !options.include_hidden && hidden(&name) {
                    return false;
                }
                !(entry.file_type().is_dir() && options.exclude.iter().any(|e| e == name.as_ref()))
            });
        for entry in walker.flatten() {
            if files.len() % 1024 == 0 {
                if is_cancelled() {
                    return Err(cancelled());
                }
                progress(files.len());
            }
            if !entry.file_type().is_file() {
                continue;
            }
            if !extensions.is_empty() {
                let ext = entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if !extensions.contains(&ext) {
                    continue;
                }
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.len() < options.min_size.max(1) {
                continue;
            }
            files.push(FileInfo {
                path: entry.into_path(),
                size: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as i64),
                inode: inode(&meta),
                root: index,
            });
        }
    }
    progress(files.len());
    Ok(files)
}

#[cfg(test)]
#[path = "scan_test.rs"]
mod tests;
