//! 遍历文件夹：按忽略规则跳过条目，记录相对路径（统一用 / 分隔）、类型、大小与修改时间。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub size: u64,
    /// 修改时间（毫秒）
    pub modified: i64,
}

pub struct Scanned {
    pub entries: BTreeMap<String, Entry>,
    /// 跳过的符号链接数
    pub links: usize,
}

/// 简单通配：* 匹配任意多个字符，? 匹配一个字符；不区分大小写
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

pub struct Ignore {
    patterns: Vec<String>,
    hidden: bool,
}

impl Ignore {
    pub fn new(patterns: &[String], hidden: bool) -> Self {
        Self {
            patterns: patterns
                .iter()
                .map(|p| p.trim().trim_end_matches('/').to_owned())
                .filter(|p| !p.is_empty())
                .collect(),
            hidden,
        }
    }

    /// 含 / 的规则匹配相对路径，否则匹配名称
    pub fn matches(&self, rel: &str) -> bool {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if self.hidden && name.starts_with('.') {
            return true;
        }
        self.patterns.iter().any(|p| {
            if p.contains('/') {
                glob(p.trim_start_matches('/'), rel)
            } else {
                glob(p, name)
            }
        })
    }
}

pub fn root(path: &str) -> PluginResult<PathBuf> {
    dunce::canonicalize(Path::new(path))
        .ok()
        .filter(|p| p.is_dir())
        .ok_or_else(|| PluginError::new("fc.not_a_folder").with("path", path))
}

pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn millis(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

pub fn scan(root: &Path, ignore: &Ignore, cancelled: &dyn Fn() -> bool) -> Option<Scanned> {
    let mut entries = BTreeMap::new();
    let mut links = 0;
    let walker = walkdir::WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !ignore.matches(&relative(root, e.path())));
    for entry in walker.flatten() {
        if cancelled() {
            return None;
        }
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            links += 1;
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let kind = if file_type.is_dir() {
            Kind::Dir
        } else {
            Kind::File
        };
        entries.insert(
            relative(root, entry.path()),
            Entry {
                kind,
                size: if kind == Kind::File { meta.len() } else { 0 },
                modified: millis(&meta),
            },
        );
    }
    Some(Scanned { entries, links })
}

#[cfg(test)]
#[path = "scan_test.rs"]
mod tests;
