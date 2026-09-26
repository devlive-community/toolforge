//! 压缩包目录：记录全部条目，补全隐含的文件夹，并按文件夹列出子项。

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::Serialize;

use crate::format::{Format, Kind, Meta};

pub struct Catalog {
    pub path: PathBuf,
    pub format: Format,
    pub password: Option<String>,
    pub entries: Vec<Meta>,
    /// 文件夹路径（根为空字符串）→ 子项名称 → 条目序号（隐含文件夹为 None）
    tree: HashMap<String, BTreeMap<String, Option<usize>>>,
    /// 文件夹路径 → (总大小, 文件数)
    totals: HashMap<String, (u64, u64)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Child {
    pub name: String,
    pub path: String,
    pub kind: Kind,
    pub size: u64,
    pub packed: Option<u64>,
    pub modified: Option<String>,
    /// 文件夹内的文件数
    pub files: u64,
    pub encrypted: bool,
    pub safe: bool,
}

fn parent_of(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        Some((parent, name)) => (parent, name),
        None => ("", path),
    }
}

impl Catalog {
    pub fn new(
        path: PathBuf,
        format: Format,
        password: Option<String>,
        entries: Vec<Meta>,
    ) -> Self {
        let mut catalog = Self {
            path,
            format,
            password,
            entries,
            tree: HashMap::new(),
            totals: HashMap::new(),
        };
        catalog.tree.insert(String::new(), BTreeMap::new());
        for index in 0..catalog.entries.len() {
            let path = catalog.entries[index].path.clone();
            catalog.insert(&path, Some(index));
            let entry = &catalog.entries[index];
            if entry.kind == Kind::Dir {
                catalog.tree.entry(path.clone()).or_default();
            } else {
                let size = entry.size;
                let mut current = path.as_str();
                loop {
                    let (parent, _) = parent_of(current);
                    let total = catalog.totals.entry(parent.to_owned()).or_default();
                    total.0 += size;
                    total.1 += 1;
                    if parent.is_empty() {
                        break;
                    }
                    current = parent;
                }
            }
        }
        catalog
    }

    /// 登记条目及其所有上级文件夹
    fn insert(&mut self, path: &str, index: Option<usize>) {
        let (parent, name) = parent_of(path);
        if !parent.is_empty() && !self.tree.contains_key(parent) {
            self.insert(parent, None);
        }
        self.tree
            .entry(parent.to_owned())
            .or_default()
            .entry(name.to_owned())
            .and_modify(|existing| {
                if existing.is_none() {
                    *existing = index;
                }
            })
            .or_insert(index);
        if index.is_none() {
            self.tree.entry(path.to_owned()).or_default();
        }
    }

    pub fn encrypted(&self) -> bool {
        self.entries.iter().any(|e| e.encrypted)
    }

    pub fn total(&self) -> (u64, u64) {
        self.totals.get("").copied().unwrap_or_default()
    }

    pub fn children(&self, dir: &str) -> Option<Vec<Child>> {
        let children = self.tree.get(dir)?;
        let mut out: Vec<Child> = children
            .iter()
            .map(|(name, index)| {
                let path = if dir.is_empty() {
                    name.clone()
                } else {
                    format!("{dir}/{name}")
                };
                let is_dir = self.tree.contains_key(&path);
                let entry = index.map(|i| &self.entries[i]);
                let (size, files) = if is_dir {
                    self.totals.get(&path).copied().unwrap_or_default()
                } else {
                    (entry.map_or(0, |e| e.size), 1)
                };
                Child {
                    name: name.clone(),
                    kind: if is_dir {
                        Kind::Dir
                    } else {
                        entry.map_or(Kind::File, |e| e.kind)
                    },
                    size,
                    packed: if is_dir {
                        None
                    } else {
                        entry.and_then(|e| e.packed)
                    },
                    modified: entry.and_then(|e| e.modified.clone()),
                    files,
                    encrypted: entry.is_some_and(|e| e.encrypted),
                    safe: entry.is_none_or(|e| e.safe),
                    path,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            (b.kind == Kind::Dir)
                .cmp(&(a.kind == Kind::Dir))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Some(out)
    }
}

#[cfg(test)]
#[path = "catalog_test.rs"]
mod tests;
