//! 磁盘空间分析插件后端：并行扫描文件夹，计算每个文件夹的大小，生成树图布局、类型统计与最大文件列表，
//! 并可把选中的项目移到回收站。

mod category;
mod tree;
mod treemap;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use category::Category;
use tree::{NodeId, Summary, Tree};

const MANIFEST: &str = include_str!("../../manifest.json");
const CAPACITY: usize = 2;
/// 列表中最多返回的子项数，其余合并
const MAX_CHILDREN: usize = 500;

#[derive(Default)]
struct Scans {
    next_id: AtomicU64,
    items: Mutex<VecDeque<(u64, Arc<RwLock<Tree>>)>>,
}

impl Scans {
    fn insert(&self, tree: Tree) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items.push_back((id, Arc::new(RwLock::new(tree))));
        while items.len() > CAPACITY {
            items.pop_front();
        }
        id
    }

    fn get(&self, id: u64) -> PluginResult<Arc<RwLock<Tree>>> {
        let items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| PluginError::new("disk.scan_expired"))
    }
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanArgs {
    root: String,
    #[serde(default = "yes")]
    include_hidden: bool,
    #[serde(default = "yes")]
    same_device: bool,
}

#[derive(Deserialize)]
struct NodeArgs {
    id: u64,
    #[serde(default)]
    node: NodeId,
}

#[derive(Deserialize)]
struct TreemapArgs {
    id: u64,
    #[serde(default)]
    node: NodeId,
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
struct LargestArgs {
    id: u64,
    #[serde(default)]
    node: NodeId,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Deserialize)]
struct TrashArgs {
    id: u64,
    nodes: Vec<NodeId>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Crumb {
    id: NodeId,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Listing {
    node: Summary,
    path: String,
    crumbs: Vec<Crumb>,
    children: Vec<Summary>,
    /// 超出上限未列出的子项
    others: Option<(usize, u64)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TypeStat {
    category: Category,
    size: u64,
    files: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LargeFile {
    id: NodeId,
    name: String,
    path: String,
    size: u64,
    category: Category,
}

fn move_to_trash(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // Finder 方式需要自动化授权并播放音效，NSFileManager 方式不需要
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(path).map_err(|e| e.to_string())
    }
}

pub struct DiskUsage {
    manifest: Manifest,
    scans: Scans,
}

impl Default for DiskUsage {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            scans: Scans::default(),
        }
    }
}

fn listing(tree: &Tree, id: NodeId) -> PluginResult<Listing> {
    let node = tree.get(id)?;
    let children: Vec<Summary> = node
        .children
        .iter()
        .take(MAX_CHILDREN)
        .map(|c| tree.summary(*c))
        .collect();
    let rest = &node.children[children.len()..];
    Ok(Listing {
        node: tree.summary(id),
        path: tree.path(id).to_string_lossy().into_owned(),
        crumbs: tree
            .ancestors(id)
            .into_iter()
            .map(|(id, name)| Crumb {
                id,
                name: name.to_owned(),
            })
            .collect(),
        others: (!rest.is_empty()).then(|| {
            (
                rest.len(),
                rest.iter().map(|c| tree.nodes[*c as usize].size).sum(),
            )
        }),
        children,
    })
}

fn types(tree: &Tree, id: NodeId) -> PluginResult<Vec<TypeStat>> {
    tree.get(id)?;
    let mut stats: Vec<TypeStat> = category::ALL
        .iter()
        .map(|c| TypeStat {
            category: *c,
            size: 0,
            files: 0,
        })
        .collect();
    for file in tree.files_under(id) {
        let node = &tree.nodes[file as usize];
        let stat = &mut stats[node.category as usize];
        stat.size += node.size;
        stat.files += 1;
    }
    stats.retain(|s| s.files > 0);
    stats.sort_by_key(|s| std::cmp::Reverse(s.size));
    Ok(stats)
}

fn largest(tree: &Tree, id: NodeId, limit: usize) -> PluginResult<Vec<LargeFile>> {
    tree.get(id)?;
    let mut files: Vec<NodeId> = tree.files_under(id).collect();
    files.sort_by_key(|f| std::cmp::Reverse(tree.nodes[*f as usize].size));
    Ok(files
        .into_iter()
        .take(limit.min(500))
        .map(|f| {
            let node = &tree.nodes[f as usize];
            LargeFile {
                id: f,
                name: node.name.to_string(),
                path: tree.path(f).to_string_lossy().into_owned(),
                size: node.size,
                category: node.category,
            }
        })
        .collect())
}

impl DiskUsage {
    fn scan(&self, args: ScanArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let started = Instant::now();
        ctx.stage("disk.scan");
        let files = AtomicU64::new(0);
        let done = AtomicBool::new(false);
        let cancelled = || ctx.is_cancelled();
        let tree = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let result = Tree::scan(
                    Path::new(&args.root),
                    args.include_hidden,
                    args.same_device,
                    &files,
                    &cancelled,
                );
                done.store(true, Ordering::Relaxed);
                result
            });
            // 总数未知，只汇报已扫描的文件数
            while !done.load(Ordering::Relaxed) {
                ctx.progress(files.load(Ordering::Relaxed), 0);
                std::thread::sleep(Duration::from_millis(200));
            }
            worker
                .join()
                .unwrap_or_else(|_| Err(PluginError::new("disk.scan_failed")))
        })?;
        let root = tree.summary(tree::ROOT);
        let denied = tree.denied;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        ctx.log(
            LogLevel::Info,
            "disk.scanned",
            json!({ "files": root.files, "bytes": root.size, "denied": denied, "ms": elapsed_ms }),
        );
        let id = self.scans.insert(tree);
        Ok(json!({ "id": id, "root": root, "denied": denied, "elapsedMs": elapsed_ms }))
    }

    fn trash(&self, args: TrashArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let scan = self.scans.get(args.id)?;
        ctx.stage("disk.trash");
        let mut removed = Vec::new();
        let mut freed = 0u64;
        let mut failed = 0usize;
        let total = args.nodes.len() as u64;
        for (index, node) in args.nodes.iter().enumerate() {
            if *node == tree::ROOT {
                return Err(PluginError::new("disk.cannot_remove_root"));
            }
            let (path, size) = {
                let tree = scan.read().unwrap_or_else(|e| e.into_inner());
                let Ok(item) = tree.get(*node) else {
                    continue;
                };
                (tree.path(*node), item.size)
            };
            match move_to_trash(&path) {
                Ok(()) => {
                    scan.write()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(*node);
                    ctx.log(
                        LogLevel::Info,
                        "disk.trashed",
                        json!({ "path": path.to_string_lossy(), "size": size }),
                    );
                    removed.push(*node);
                    freed += size;
                }
                Err(detail) => {
                    failed += 1;
                    ctx.log(
                        LogLevel::Error,
                        "disk.trash_failed",
                        json!({ "path": path.to_string_lossy(), "detail": detail }),
                    );
                }
            }
            ctx.progress(index as u64 + 1, total);
        }
        Ok(json!({ "removed": removed, "freed": freed, "failed": failed }))
    }
}

impl ToolPlugin for DiskUsage {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => {
                let args: NodeArgs = parse_args(args)?;
                let scan = self.scans.get(args.id)?;
                let tree = scan.read().unwrap_or_else(|e| e.into_inner());
                to_value(listing(&tree, args.node)?)
            }
            "treemap" => {
                let args: TreemapArgs = parse_args(args)?;
                let scan = self.scans.get(args.id)?;
                let tree = scan.read().unwrap_or_else(|e| e.into_inner());
                tree.get(args.node)?;
                to_value(treemap::tiles(
                    &tree,
                    args.node,
                    args.width.clamp(1.0, 8000.0),
                    args.height.clamp(1.0, 8000.0),
                ))
            }
            "types" => {
                let args: NodeArgs = parse_args(args)?;
                let scan = self.scans.get(args.id)?;
                let tree = scan.read().unwrap_or_else(|e| e.into_inner());
                to_value(types(&tree, args.node)?)
            }
            "largest" => {
                let args: LargestArgs = parse_args(args)?;
                let scan = self.scans.get(args.id)?;
                let tree = scan.read().unwrap_or_else(|e| e.into_inner());
                to_value(largest(&tree, args.node, args.limit)?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "scan" => self.scan(parse_args(args)?, ctx),
            "trash" => self.trash(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
