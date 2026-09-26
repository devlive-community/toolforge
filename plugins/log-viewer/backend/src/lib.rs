//! 日志查看插件后端：为大文件建立行索引，按级别与文本 / 正则筛选，跟随文件增长（tail -f）。

mod doc;
mod index;
mod level;
mod text;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use doc::{Doc, Stats};
use text::Filter;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 同时保留的文件数量，更早的会被释放
const CAPACITY: usize = 3;
const MAX_PAGE: usize = 1000;

#[derive(Default)]
struct Docs {
    next_id: AtomicU64,
    items: Mutex<VecDeque<(u64, Arc<Doc>)>>,
}

impl Docs {
    fn insert(&self, doc: Doc) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items.push_back((id, Arc::new(doc)));
        while items.len() > CAPACITY {
            items.pop_front();
        }
        id
    }

    fn get(&self, id: u64) -> PluginResult<Arc<Doc>> {
        let items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, d)| d.clone())
            .ok_or_else(|| PluginError::new("log.file_expired"))
    }
}

#[derive(Deserialize)]
struct OpenArgs {
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenInfo {
    id: u64,
    name: String,
    path: String,
    encoding: String,
    #[serde(flatten)]
    stats: Stats,
    elapsed_ms: u64,
}

#[derive(Deserialize)]
struct FilterArgs {
    id: u64,
    #[serde(flatten)]
    filter: Filter,
}

#[derive(Deserialize)]
struct PageArgs {
    id: u64,
    #[serde(default)]
    view: u64,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    200
}

#[derive(Deserialize)]
struct LineArgs {
    id: u64,
    line: usize,
}

#[derive(Deserialize)]
struct IdArgs {
    id: u64,
}

pub struct LogViewer {
    manifest: Manifest,
    docs: Docs,
}

impl Default for LogViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            docs: Docs::default(),
        }
    }
}

impl LogViewer {
    fn open(&self, args: OpenArgs, ctx: &dyn TaskContext) -> PluginResult<OpenInfo> {
        let started = Instant::now();
        let path = Path::new(&args.path);
        ctx.stage("log.index");
        let mut last = 0u64;
        let doc = Doc::open(
            path,
            |done, total| {
                // 每 8 MB 汇报一次进度
                if done - last >= 8 * 1024 * 1024 || done == total {
                    last = done;
                    ctx.progress(done, total);
                }
            },
            || ctx.is_cancelled(),
        )?;
        let stats = doc.stats();
        let encoding = doc.encoding.name().to_owned();
        let elapsed_ms = started.elapsed().as_millis() as u64;
        ctx.log(
            LogLevel::Info,
            "log.indexed",
            json!({ "lines": stats.lines, "bytes": stats.bytes, "encoding": encoding, "ms": elapsed_ms }),
        );
        let id = self.docs.insert(doc);
        Ok(OpenInfo {
            id,
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: args.path,
            encoding,
            stats,
            elapsed_ms,
        })
    }

    fn filter(&self, args: FilterArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let started = Instant::now();
        let doc = self.docs.get(args.id)?;
        ctx.stage("log.filter");
        let cancelled = || ctx.is_cancelled();
        let (view, total) = doc.filter(
            &args.filter,
            |done, all| ctx.progress(done, all),
            &cancelled,
        )?;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        ctx.log(
            LogLevel::Info,
            "log.filtered",
            json!({ "matches": total, "ms": elapsed_ms }),
        );
        Ok(json!({ "view": view, "total": total, "elapsedMs": elapsed_ms }))
    }
}

impl ToolPlugin for LogViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "lines" => {
                let args: PageArgs = parse_args(args)?;
                let doc = self.docs.get(args.id)?;
                to_value(doc.page(args.view, args.offset, args.limit.min(MAX_PAGE))?)
            }
            "line" => {
                let args: LineArgs = parse_args(args)?;
                to_value(self.docs.get(args.id)?.detail(args.line)?)
            }
            "refresh" => {
                let args: IdArgs = parse_args(args)?;
                to_value(self.docs.get(args.id)?.refresh()?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "open" => to_value(self.open(parse_args(args)?, ctx)?),
            "filter" => self.filter(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}
