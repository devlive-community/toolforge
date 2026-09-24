//! CSV 查看器插件后端：流式读取大文件、识别编码与分隔符，表格保存在 Rust 侧，
//! 前端按页读取当前视图（排序、搜索、筛选后）的行。

mod export;
mod load;
mod table;
mod view;

use std::collections::VecDeque;
use std::io::{BufWriter, Read};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use load::{Detected, Options};
use table::{Column, Table};
use view::Spec;

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_FILE: u64 = 1024 * 1024 * 1024;
const MAX_TEXT: usize = 20 * 1024 * 1024;
/// 同时保留的表格数量，更早的会被释放
const CAPACITY: usize = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TableInfo {
    id: u64,
    name: Option<String>,
    path: Option<String>,
    rows: usize,
    columns: Vec<Column>,
    #[serde(flatten)]
    detected: Detected,
    bytes: u64,
    elapsed_ms: u64,
}

struct Loaded {
    table: Table,
    has_header: bool,
    /// 最近一次视图，翻页时复用
    view: Mutex<Option<(Spec, Arc<Vec<u32>>)>>,
}

#[derive(Default)]
struct Tables {
    next_id: AtomicU64,
    items: Mutex<VecDeque<(u64, Arc<Loaded>)>>,
}

impl Tables {
    fn insert(&self, loaded: Loaded) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items.push_back((id, Arc::new(loaded)));
        while items.len() > CAPACITY {
            items.pop_front();
        }
        id
    }

    fn get(&self, id: u64) -> PluginResult<Arc<Loaded>> {
        let items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| PluginError::new("csv.table_expired"))
    }
}

impl Loaded {
    fn view(&self, spec: &Spec) -> PluginResult<Option<Arc<Vec<u32>>>> {
        if spec.is_identity() {
            view::build(&self.table, spec)?;
            return Ok(None);
        }
        let mut cached = self.view.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((key, rows)) = cached.as_ref()
            && key == spec
        {
            return Ok(Some(rows.clone()));
        }
        let rows = Arc::new(view::build(&self.table, spec)?.unwrap_or_default());
        *cached = Some((spec.clone(), rows.clone()));
        Ok(Some(rows))
    }
}

#[derive(Deserialize)]
struct OpenArgs {
    path: String,
    #[serde(flatten)]
    options: Options,
}

#[derive(Deserialize)]
struct TextArgs {
    text: String,
    #[serde(flatten)]
    options: Options,
}

#[derive(Deserialize)]
struct RowsArgs {
    id: u64,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(flatten)]
    spec: Spec,
}

fn default_limit() -> usize {
    200
}

#[derive(Deserialize)]
struct StatsArgs {
    id: u64,
    column: usize,
    #[serde(flatten)]
    spec: Spec,
}

#[derive(Deserialize)]
struct ExportArgs {
    id: u64,
    path: String,
    format: export::Format,
    #[serde(flatten)]
    spec: Spec,
}

pub struct CsvViewer {
    manifest: Manifest,
    tables: Tables,
}

impl Default for CsvViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            tables: Tables::default(),
        }
    }
}

impl CsvViewer {
    /// 解码、解析并登记表格
    fn load(
        &self,
        bytes: Vec<u8>,
        options: &Options,
        path: Option<&str>,
        progress: impl FnMut(u64),
        is_cancelled: impl Fn() -> bool,
        start: Instant,
    ) -> PluginResult<TableInfo> {
        let size = bytes.len() as u64;
        let (text, encoding) = load::decode(bytes, options.encoding.as_deref())?;
        let delimiter = load::parse_delimiter(options.delimiter.as_deref())?
            .unwrap_or_else(|| load::detect_delimiter(&text));
        let mut table = load::parse(&text, delimiter, progress, is_cancelled)?;
        drop(text);
        let has_header = load::finish(&mut table, options.header);
        let info = TableInfo {
            id: 0,
            name: path
                .and_then(|p| Path::new(p).file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            path: path.map(str::to_owned),
            rows: table.rows(),
            columns: table.columns.clone(),
            detected: Detected {
                delimiter: load::delimiter_name(delimiter),
                encoding,
                has_header,
            },
            bytes: size,
            elapsed_ms: start.elapsed().as_millis() as u64,
        };
        let id = self.tables.insert(Loaded {
            table,
            has_header,
            view: Mutex::new(None),
        });
        Ok(TableInfo { id, ..info })
    }

    fn open(&self, args: OpenArgs, ctx: &dyn TaskContext) -> PluginResult<TableInfo> {
        let start = Instant::now();
        let size = ctx.file_size(&args.path)?;
        if size > MAX_FILE {
            return Err(PluginError::new("csv.too_large").with("limit", "1 GB"));
        }
        ctx.stage("csv.read");
        let mut reader = ctx.open_file(&args.path)?;
        let mut bytes = Vec::with_capacity(size as usize);
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let n = reader
                .read(&mut buffer)
                .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
            ctx.progress(bytes.len() as u64, size.max(1) * 2);
        }
        ctx.stage("csv.parse");
        let total = size.max(1);
        let info = self.load(
            bytes,
            &args.options,
            Some(&args.path),
            |done| ctx.progress(total + done.min(total), total * 2),
            || ctx.is_cancelled(),
            start,
        )?;
        ctx.log(
            LogLevel::Info,
            "csv.loaded",
            json!({
                "rows": info.rows,
                "columns": info.columns.len(),
                "encoding": info.detected.encoding,
                "delimiter": info.detected.delimiter,
                "ms": info.elapsed_ms,
            }),
        );
        Ok(info)
    }

    fn export(&self, args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let loaded = self.tables.get(args.id)?;
        let view = loaded.view(&args.spec)?;
        let total = view::view_len(&loaded.table, view.as_deref().map(Vec::as_slice));
        ctx.stage("csv.export");
        let file = std::fs::File::create(&args.path)
            .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
        let mut out = BufWriter::new(file);
        let rows = export::write(
            &loaded.table,
            view.as_deref().map(Vec::as_slice),
            args.format,
            loaded.has_header,
            &mut out,
            |done| ctx.progress(done as u64, total.max(1) as u64),
            || ctx.is_cancelled(),
        );
        let rows = match rows {
            Ok(rows) => rows,
            Err(err) => {
                drop(out);
                let _ = std::fs::remove_file(&args.path);
                return Err(err);
            }
        };
        std::io::Write::flush(&mut out)
            .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
        ctx.log(
            LogLevel::Info,
            "csv.exported",
            json!({ "rows": rows, "path": args.path }),
        );
        Ok(json!({ "rows": rows, "path": args.path }))
    }
}

impl ToolPlugin for CsvViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse_text" => {
                let args: TextArgs = parse_args(args)?;
                if args.text.len() > MAX_TEXT {
                    return Err(PluginError::new("csv.too_large").with("limit", "20 MB"));
                }
                if args.text.trim().is_empty() {
                    return Err(PluginError::new("csv.empty"));
                }
                to_value(self.load(
                    args.text.into_bytes(),
                    &args.options,
                    None,
                    |_| {},
                    || false,
                    Instant::now(),
                )?)
            }
            "rows" => {
                let args: RowsArgs = parse_args(args)?;
                let loaded = self.tables.get(args.id)?;
                let view = loaded.view(&args.spec)?;
                to_value(view::page(
                    &loaded.table,
                    view.as_deref().map(Vec::as_slice),
                    args.offset,
                    args.limit,
                ))
            }
            "stats" => {
                let args: StatsArgs = parse_args(args)?;
                let loaded = self.tables.get(args.id)?;
                let view = loaded.view(&args.spec)?;
                to_value(view::stats(
                    &loaded.table,
                    view.as_deref().map(Vec::as_slice),
                    args.column,
                )?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "open" => to_value(self.open(parse_args(args)?, ctx)?),
            "export" => self.export(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
