//! 电子表格查看器插件后端：读取 xlsx / xlsm / xlsb / xls / ods 的所有工作表，
//! 表格保存在 Rust 侧，前端按页读取当前视图（排序、搜索、筛选后）的行。

mod convert;
mod export;
mod table;
mod view;

use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use calamine::{Reader, Sheets, XlsxError, open_workbook_auto_from_rs};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use table::Table;
use view::Spec;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 沙箱内存上限为 1 GB：原文件与表格同时在内存中，实测 66 MB、2500 万单元格的 xlsx 峰值约 700 MB
const MAX_FILE: u64 = 64 * 1024 * 1024;
/// xlsx 工作表声明的单元格数上限；共享字符串让每 MB 的单元格更多，只看文件大小不够
const MAX_CELLS: u64 = 25_000_000;
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

    fn remove(&self, id: u64) {
        let mut items = self.items.lock().unwrap_or_else(|e| e.into_inner());
        items.retain(|(i, _)| *i != id);
    }
}

#[derive(Deserialize)]
struct OpenArgs {
    path: String,
    /// 首行是否为表头；省略时视为有表头
    #[serde(default)]
    header: Option<bool>,
}

/// 一次性搜索（命令行与 Agent）：打开、按文本搜索一个工作表并把一页结果按指定格式输出，不保留会话
#[derive(Deserialize)]
struct SearchArgs {
    path: String,
    #[serde(default)]
    header: Option<bool>,
    #[serde(default)]
    sheet: usize,
    #[serde(default)]
    query: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_format")]
    format: export::Format,
}

fn default_format() -> export::Format {
    export::Format::Csv
}

/// 一次性转换（命令行与 Agent）：把一个工作表导出为文件
#[derive(Deserialize)]
struct ConvertArgs {
    path: String,
    #[serde(default)]
    header: Option<bool>,
    #[serde(default)]
    sheet: usize,
    output: String,
    format: export::Format,
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

fn read_failed(error: impl std::fmt::Display) -> PluginError {
    PluginError::new("xlsx.read_failed").with("detail", error.to_string())
}

/// 读取一个工作表：xlsx 逐个单元格流式构建，其余格式与单元格乱序的 xlsx 经 Range 构建
fn read_sheet(
    workbook: &mut Sheets<Cursor<Vec<u8>>>,
    name: &str,
    header: bool,
    ctx: &dyn TaskContext,
) -> PluginResult<Table> {
    if let Sheets::Xlsx(xlsx) = workbook {
        let streamed = match xlsx.worksheet_cells_reader(name) {
            Ok(mut cells) => {
                if cells.dimensions().len() > MAX_CELLS {
                    return Err(PluginError::new("xlsx.too_many_cells")
                        .with("sheet", name)
                        .with("limit", "25,000,000"));
                }
                convert::stream_table(&mut cells, header, || ctx.is_cancelled())
            }
            // 图表页等非工作表按空表处理，与 worksheet_range 一致
            Err(XlsxError::NotAWorksheet(_)) => Ok(Some(Table::new())),
            Err(error) => Err(error.into()),
        };
        match streamed {
            Ok(Some(table)) => return Ok(table),
            Ok(None) => {}
            Err(convert::StreamError::Cancelled) => return Err(cancelled()),
            Err(convert::StreamError::Read(error)) => return Err(read_failed(error)),
        }
    }
    let range = workbook.worksheet_range(name).map_err(read_failed)?;
    Ok(convert::build_table(&range, header))
}

impl XlsxViewer {
    fn open(&self, args: OpenArgs, ctx: &dyn TaskContext) -> PluginResult<WorkbookInfo> {
        let start = Instant::now();
        let size = ctx.file_size(&args.path)?;
        if size > MAX_FILE {
            return Err(PluginError::new("xlsx.too_large").with("limit", "64 MB"));
        }
        ctx.stage("xlsx.read");
        let bytes = tf_plugin_guest::net::read_to_end(&args.path, MAX_FILE)?;
        // 没有文件扩展名可用，按内容识别格式
        let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
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
            let table = read_sheet(&mut workbook, name, header, ctx)?;
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

    fn search(&self, args: SearchArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let info = self.open(
            OpenArgs {
                path: args.path,
                header: args.header,
            },
            ctx,
        )?;
        let workbook = self.workbooks.get(info.id)?;
        self.workbooks.remove(info.id);
        let sheet = workbook.sheet(args.sheet)?;
        let spec = Spec {
            query: args.query,
            ..Spec::default()
        };
        let view = sheet.view(&spec)?;
        let view = view.as_deref().map(Vec::as_slice);
        let total = view::view_len(&sheet.table, view);
        let end = args
            .offset
            .saturating_add(args.limit.min(view::MAX_PAGE))
            .min(total);
        let rows: Vec<u32> = (args.offset.min(end)..end)
            .map(|position| view::row_at(view, position) as u32)
            .collect();
        let mut out = Vec::new();
        export::write(
            &sheet.table,
            Some(&rows),
            args.format,
            sheet.has_header,
            &mut out,
            |_| {},
            || ctx.is_cancelled(),
        )?;
        let sheets: Vec<&str> = workbook
            .sheets
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        Ok(json!({
            "sheets": sheets,
            "total": total,
            "rows": rows.len(),
            "text": String::from_utf8_lossy(&out),
        }))
    }

    fn convert(&self, args: ConvertArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let info = self.open(
            OpenArgs {
                path: args.path,
                header: args.header,
            },
            ctx,
        )?;
        let result = self.export(
            ExportArgs {
                id: info.id,
                sheet: args.sheet,
                path: args.output,
                format: args.format,
                spec: Spec::default(),
            },
            ctx,
        );
        self.workbooks.remove(info.id);
        result
    }

    fn export(&self, args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let workbook = self.workbooks.get(args.id)?;
        let sheet = workbook.sheet(args.sheet)?;
        let view = sheet.view(&args.spec)?;
        let total = view::view_len(&sheet.table, view.as_deref().map(Vec::as_slice));
        ctx.stage("xlsx.export");
        // 先在内存中写好再一次写出，取消或出错时不会留下半个文件
        let mut out = Vec::new();
        let rows = export::write(
            &sheet.table,
            view.as_deref().map(Vec::as_slice),
            args.format,
            sheet.has_header,
            &mut out,
            |done| ctx.progress(done as u64, total.max(1) as u64),
            || ctx.is_cancelled(),
        );
        let rows = rows?;
        tf_plugin_guest::net::write_file(&args.path, &out)?;
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
            "search" => self.search(parse_args(args)?, ctx),
            "convert" => self.convert(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(XlsxViewer);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
