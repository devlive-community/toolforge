//! 扫描结果：所有文件与文件夹保存在一个扁平数组中，子项按大小降序排列。

#[cfg(unix)]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult, cancelled};

use crate::category::{self, Category};

pub type NodeId = u32;
pub const ROOT: NodeId = 0;

#[derive(Debug)]
pub struct Node {
    pub name: Box<str>,
    pub parent: NodeId,
    pub size: u64,
    /// 其中的文件数（文件自身为 1）
    pub files: u64,
    pub dir: bool,
    pub category: Category,
    pub children: Vec<NodeId>,
    /// 被移到回收站后标记为已删除，不再出现在结果中
    pub removed: bool,
}

pub struct Tree {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
    /// 无权限读取的文件夹数量
    pub denied: u64,
}

/// 扫描中间结构
enum Entry {
    File {
        name: Box<str>,
        size: u64,
    },
    Dir {
        name: Box<str>,
        children: Vec<Entry>,
    },
}

struct Walker<'a> {
    device: Option<u64>,
    include_hidden: bool,
    files: &'a AtomicU64,
    denied: &'a AtomicU64,
    /// 硬链接只计算一次（Windows 上无法取得 inode，不做去重）
    #[cfg(unix)]
    links: Mutex<HashSet<(u64, u64)>>,
    is_cancelled: &'a (dyn Fn() -> bool + Sync),
}

#[cfg(unix)]
fn device(meta: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.dev())
}

#[cfg(not(unix))]
fn device(_: &std::fs::Metadata) -> Option<u64> {
    None
}

impl Walker<'_> {
    /// 硬链接的第二次及以后出现按 0 字节计算
    fn counted_size(&self, meta: &std::fs::Metadata) -> u64 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 {
                let mut links = self.links.lock().unwrap_or_else(|e| e.into_inner());
                if !links.insert((meta.dev(), meta.ino())) {
                    return 0;
                }
            }
        }
        meta.len()
    }

    fn walk(&self, path: &Path, name: Box<str>) -> Entry {
        let Ok(read) = std::fs::read_dir(path) else {
            self.denied.fetch_add(1, Ordering::Relaxed);
            return Entry::Dir {
                name,
                children: Vec::new(),
            };
        };
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        for entry in read.flatten() {
            let child_name: Box<str> = entry.file_name().to_string_lossy().into();
            if !self.include_hidden && child_name.starts_with('.') {
                continue;
            }
            // 不跟随符号链接
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                // 不进入其他文件系统（外接磁盘、网络挂载等）
                if self.device.is_some() && device(&meta) != self.device {
                    continue;
                }
                dirs.push((entry.path(), child_name));
            } else if meta.is_file() {
                self.files.fetch_add(1, Ordering::Relaxed);
                files.push(Entry::File {
                    name: child_name,
                    size: self.counted_size(&meta),
                });
            }
        }
        if (self.is_cancelled)() {
            return Entry::Dir {
                name,
                children: files,
            };
        }
        let subdirs: Vec<Entry> = dirs
            .into_par_iter()
            .map(|(path, name)| self.walk(&path, name))
            .collect();
        files.extend(subdirs);
        Entry::Dir {
            name,
            children: files,
        }
    }
}

impl Tree {
    pub fn scan(
        root: &Path,
        include_hidden: bool,
        same_device: bool,
        files: &AtomicU64,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> PluginResult<Self> {
        let root = root
            .canonicalize()
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| {
                PluginError::new("disk.not_a_folder").with("path", root.to_string_lossy().as_ref())
            })?;
        let meta = std::fs::metadata(&root)
            .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
        let denied = AtomicU64::new(0);
        let walker = Walker {
            device: if same_device { device(&meta) } else { None },
            include_hidden,
            files,
            denied: &denied,
            #[cfg(unix)]
            links: Mutex::new(HashSet::new()),
            is_cancelled,
        };
        let entry = walker.walk(&root, root.to_string_lossy().into());
        if is_cancelled() {
            return Err(cancelled());
        }
        let mut tree = Tree {
            root,
            nodes: Vec::new(),
            denied: denied.into_inner(),
        };
        tree.flatten(entry, ROOT);
        Ok(tree)
    }

    fn flatten(&mut self, entry: Entry, parent: NodeId) -> NodeId {
        let id = self.nodes.len() as NodeId;
        match entry {
            Entry::File { name, size } => {
                self.nodes.push(Node {
                    category: category::of(&name),
                    name,
                    parent,
                    size,
                    files: 1,
                    dir: false,
                    children: Vec::new(),
                    removed: false,
                });
            }
            Entry::Dir { name, children } => {
                self.nodes.push(Node {
                    name,
                    parent,
                    size: 0,
                    files: 0,
                    dir: true,
                    category: Category::Other,
                    children: Vec::new(),
                    removed: false,
                });
                let ids: Vec<NodeId> = children.into_iter().map(|c| self.flatten(c, id)).collect();
                let (size, files) = ids.iter().fold((0, 0), |(s, f), c| {
                    (
                        s + self.nodes[*c as usize].size,
                        f + self.nodes[*c as usize].files,
                    )
                });
                let mut ids = ids;
                ids.sort_by(|a, b| {
                    let (a, b) = (&self.nodes[*a as usize], &self.nodes[*b as usize]);
                    b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name))
                });
                let node = &mut self.nodes[id as usize];
                node.children = ids;
                node.size = size;
                node.files = files;
            }
        }
        id
    }

    pub fn get(&self, id: NodeId) -> PluginResult<&Node> {
        self.nodes
            .get(id as usize)
            .filter(|n| !n.removed)
            .ok_or_else(|| PluginError::new("disk.node_missing"))
    }

    /// 从根到该节点的路径（节点 id 与名称）
    pub fn ancestors(&self, id: NodeId) -> Vec<(NodeId, &str)> {
        let mut chain = Vec::new();
        let mut current = id;
        loop {
            chain.push((current, &*self.nodes[current as usize].name));
            if current == ROOT {
                break;
            }
            current = self.nodes[current as usize].parent;
        }
        chain.reverse();
        chain
    }

    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut path = self.root.clone();
        for (_, name) in self.ancestors(id).into_iter().skip(1) {
            path.push(name);
        }
        path
    }

    /// 移除节点并从所有上级文件夹中扣除大小
    pub fn remove(&mut self, id: NodeId) {
        if id == ROOT || self.nodes[id as usize].removed {
            return;
        }
        let (size, files, parent) = {
            let node = &self.nodes[id as usize];
            (node.size, node.files, node.parent)
        };
        self.nodes[id as usize].removed = true;
        self.nodes[parent as usize].children.retain(|c| *c != id);
        let mut current = parent;
        loop {
            let node = &mut self.nodes[current as usize];
            node.size = node.size.saturating_sub(size);
            node.files = node.files.saturating_sub(files);
            if current == ROOT {
                break;
            }
            current = node.parent;
        }
    }

    /// 子树中的所有文件（深度优先）
    pub fn files_under(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let mut stack = vec![id];
        std::iter::from_fn(move || {
            while let Some(current) = stack.pop() {
                let node = &self.nodes[current as usize];
                if node.dir {
                    stack.extend(node.children.iter().copied());
                } else {
                    return Some(current);
                }
            }
            None
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: NodeId,
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub dir: bool,
    pub category: Category,
    /// 直接子项数量
    pub items: usize,
    pub path: String,
}

impl Tree {
    pub fn summary(&self, id: NodeId) -> Summary {
        let node = &self.nodes[id as usize];
        Summary {
            id,
            name: node.name.to_string(),
            size: node.size,
            files: node.files,
            dir: node.dir,
            category: node.category,
            items: node.children.len(),
            path: self.path(id).to_string_lossy().into_owned(),
        }
    }
}

#[cfg(test)]
#[path = "tree_test.rs"]
mod tests;
