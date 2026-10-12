//! 电子表格查看器插件后端：读取 xlsx / xlsm / xlsb / xls / ods 的所有工作表，
//! 表格保存在 Rust 侧，前端按页读取当前视图（排序、搜索、筛选后）的行。

mod convert;
mod export;
mod table;
mod view;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use calamine::{Reader, open_workbook_auto};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use table::Table;
use view::Spec;

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_FILE: u64 = 512 * 1024 * 1024;
/// 同时保留的工作簿数量，更早的会被释放
const CAPACITY: usize = 2;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SheetInfo {
    name: String,
    rows: usize,
    columns: Vec<table::Column>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookInfo {
    id: u64,
    name: Option<String>,
    path: Option<String>,
    sheets: Vec<SheetInfo>,
    bytes: u64,
    elapsed_ms: u64,
}

pub struct Sheet {
    pub table: Table,
    pub has_header: bool,
    /// 最近一次视图，翻页时复用
    view: Mutex<Option<(Spec, Arc<Vec<u32>>)>>,
}

impl Sheet {
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

struct Workbook {
    name: Option<String>,
    path: Option<String>,
    sheets: Vec<(String, Sheet)>,
    bytes: u64,
    elapsed_ms: u64,
}

impl Workbook {
    fn sheet(&self, index: usize) -> PluginResult<&Sheet> {
        self.sheets
            .get(index)
            .map(|(_, s)| s)
            .ok_or_else(|| PluginError::new("xlsx.invalid_sheet").with("sheet", index))
    }

    fn info(&self, id: u64) -> WorkbookInfo {
        WorkbookInfo {
            id,
            name: self.name.clone(),
            path: self.path.clone(),
            bytes: self.bytes,
            elapsed_ms: self.elapsed_ms,
            sheets: self
                .sheets
                .iter()
                .map(|(name, sheet)| SheetInfo {
                    name: name.clone(),
                    rows: sheet.table.rows(),
                    columns: sheet.table.columns.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Default)]
struct Workbooks {
    next_id: AtomicU64,
    items: Mutex<VecDeque<(u64, Arc<Workbook>)>>,
}

impl Workbooks {
    fn insert(&self, workbook: Workbook) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items.push_back((id, Arc::new(workbook)));
        while items.len() > CAPACITY {
            items.pop_front();
        }
        id
    }

    fn get(&self, id: u64) -> PluginResult<Arc<Workbook>> {
        let items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, w)| w.clone())
            .ok_or_else(|| PluginError::new("xlsx.table_expired"))
    }
}

#[derive(Deserialize)]
struct OpenArgs {
    path: String,
    /// 首行是否为表头；省略时视为有表头
    #[serde(default)]
    header: Option<bool>,
}

#[derive(Deserialize)]
struct RowsArgs {
    id: u64,
    sheet: usize,
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
    sheet: usize,
    column: usize,
    #[serde(flatten)]
    spec: Spec,
}

#[derive(Deserialize)]
struct ExportArgs {
    id: u64,
    sheet: usize,
    path: String,
    format: export::Format,
    #[serde(flatten)]
    spec: Spec,
}

pub struct XlsxViewer {
    manifest: Manifest,
    workbooks: Workbooks,
}

impl Default for XlsxViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            workbooks: Workbooks::default(),
        }
    }
}

impl XlsxViewer {
    fn open(&self, args: OpenArgs, ctx: &dyn TaskContext) -> PluginResult<WorkbookInfo> {
        let start = Instant::now();
        let size = ctx.file_size(&args.path)?;
        if size > MAX_FILE {
            return Err(PluginError::new("xlsx.too_large").with("limit", "512 MB"));
        }
        ctx.stage("xlsx.read");
        let mut workbook = open_workbook_auto(&args.path)
            .map_err(|e| PluginError::new("xlsx.read_failed").with("detail", e.to_string()))?;
        let names = workbook.sheet_names().clone();
        ctx.stage("xlsx.parse");
        let total = names.len().max(1) as u64;
        let header = args.header.unwrap_or(true);
        let mut sheets = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            let range = workbook
                .worksheet_range(name)
                .map_err(|e| PluginError::new("xlsx.read_failed").with("detail", e.to_string()))?;
            let table = convert::build_table(&range, header);
            drop(range);
            ctx.progress(i as u64 + 1, total);
            // 只要存在非空单元格就保留：只有表头时数据为 0 行是正常情况
            if table.width() == 0 {
                continue;
            }
            sheets.push((
                name.clone(),
                Sheet {
                    table,
                    has_header: header,
                    view: Mutex::new(None),
                },
            ));
        }
        if sheets.is_empty() {
            return Err(PluginError::new("xlsx.empty_workbook"));
        }
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let workbook = Workbook {
            name: std::path::Path::new(&args.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned()),
            path: Some(args.path),
            sheets,
            bytes: size,
            elapsed_ms,
        };
        let rows: usize = workbook.sheets.iter().map(|(_, s)| s.table.rows()).sum();
        let columns: usize = workbook.sheets.iter().map(|(_, s)| s.table.width()).sum();
        ctx.log(
            LogLevel::Info,
            "xlsx.loaded",
            json!({
                "sheets": workbook.sheets.len(),
                "rows": rows,
                "columns": columns,
                "ms": elapsed_ms,
            }),
        );
        let id = self.workbooks.insert(workbook);
        Ok(self.workbooks.get(id)?.info(id))
    }

    fn export(&self, args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let workbook = self.workbooks.get(args.id)?;
        let sheet = workbook.sheet(args.sheet)?;
        let view = sheet.view(&args.spec)?;
        let total = view::view_len(&sheet.table, view.as_deref().map(Vec::as_slice));
        ctx.stage("xlsx.export");
        let file = std::fs::File::create(&args.path)
            .map_err(|e| PluginError::new("fs.io").with("detail", e.to_string()))?;
        let mut out = std::io::BufWriter::new(file);
        let rows = export::write(
            &sheet.table,
            view.as_deref().map(Vec::as_slice),
            args.format,
            sheet.has_header,
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
            "xlsx.exported",
            json!({ "rows": rows, "path": args.path }),
        );
        Ok(json!({ "rows": rows, "path": args.path }))
    }
}

impl ToolPlugin for XlsxViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "rows" => {
                let args: RowsArgs = parse_args(args)?;
                let workbook = self.workbooks.get(args.id)?;
                let sheet = workbook.sheet(args.sheet)?;
                let view = sheet.view(&args.spec)?;
                to_value(view::page(
                    &sheet.table,
                    view.as_deref().map(Vec::as_slice),
                    args.offset,
                    args.limit,
                ))
            }
            "stats" => {
                let args: StatsArgs = parse_args(args)?;
                let workbook = self.workbooks.get(args.id)?;
                let sheet = workbook.sheet(args.sheet)?;
                let view = sheet.view(&args.spec)?;
                to_value(view::stats(
                    &sheet.table,
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
