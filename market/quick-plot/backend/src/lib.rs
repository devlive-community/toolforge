//! 数据绘图器（在线安装插件）：粘贴或打开 CSV / TSV / JSON / JSON Lines，自动识别分隔符、表头与列类型，
//! 生成折线、柱状、散点图的绘图模型（多 Y 轴、对数坐标、时间轴），数据全程在本地处理。
//!
//! 最近一次解析的表格缓存在插件实例中，后续出图按 token 取用；实例重建后 token 失效，界面会重新解析。

mod axis;
mod infer;
mod plot;
mod table;

use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use infer::Kind;
use table::{Delimiter, Format, HeaderMode, Note, Table};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 文件大小上限
const MAX_FILE: u64 = 64 * 1024 * 1024;
/// 读取文件的块大小
const CHUNK: usize = 1 << 20;
/// 预览的行数与单元格长度
const PREVIEW_ROWS: usize = 100;
const PREVIEW_CELL: usize = 200;
/// 默认最多选几个数值列作为系列
const SUGGESTED_SERIES: usize = 4;
/// 类目少于这个数时文本 X 轴默认画柱状图
const BAR_CATEGORIES: usize = 60;
/// 导出 PNG 的大小上限（Base64 解码后）
const MAX_PNG: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParseArgs {
    text: String,
    #[serde(default)]
    format: Format,
    #[serde(default)]
    header: HeaderMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadArgs {
    path: String,
    #[serde(default)]
    format: Format,
    #[serde(default)]
    header: HeaderMode,
}

#[derive(Deserialize)]
struct SaveArgs {
    path: String,
    /// PNG 的 Base64（可带 data URL 前缀）
    data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ColumnInfo {
    index: usize,
    name: String,
    kind: Kind,
    /// 非空单元格数
    present: usize,
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Suggestion {
    chart: plot::Chart,
    x: Option<usize>,
    series: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    token: u64,
    format: Format,
    delimiter: Option<Delimiter>,
    header: bool,
    json_path: Option<String>,
    rows: usize,
    columns: Vec<ColumnInfo>,
    preview: Vec<Vec<String>>,
    notes: Vec<Note>,
    suggestion: Suggestion,
    /// 打开文件时的文件名
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

/// 最近一次解析的表格
#[derive(Default)]
struct Cache(Mutex<Option<(u64, Arc<Table>)>>);

impl Cache {
    fn store(&self, table: Table) -> u64 {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let token = cache.as_ref().map_or(1, |(t, _)| t + 1);
        *cache = Some((token, Arc::new(table)));
        token
    }

    fn get(&self, token: u64) -> PluginResult<Arc<Table>> {
        match self.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Some((t, table)) if *t == token => Ok(table.clone()),
            _ => Err(PluginError::new("plot.expired")),
        }
    }
}

fn truncate(cell: &str) -> String {
    match cell.char_indices().nth(PREVIEW_CELL) {
        Some((i, _)) => format!("{}…", &cell[..i]),
        None => cell.to_owned(),
    }
}

/// 递增（允许少量相等）的数值列适合作 X 轴
fn increasing(values: &[Option<f64>]) -> bool {
    let present: Vec<f64> = values.iter().flatten().copied().collect();
    present.len() >= 2
        && present.windows(2).all(|w| w[1] >= w[0])
        && present.first() != present.last()
}

fn suggest(table: &Table) -> Suggestion {
    let columns = &table.columns;
    let x = columns
        .iter()
        .position(|c| c.kind == Kind::Time)
        .or_else(|| {
            columns
                .first()
                .filter(|c| c.kind == Kind::Number && increasing(&c.values))
                .map(|_| 0)
        })
        .or_else(|| {
            columns
                .first()
                .filter(|c| c.kind == Kind::Text && table.rows <= BAR_CATEGORIES)
                .map(|_| 0)
        });
    let series = columns
        .iter()
        .enumerate()
        .filter(|(i, c)| c.kind == Kind::Number && Some(*i) != x)
        .map(|(i, _)| i)
        .take(SUGGESTED_SERIES)
        .collect();
    let chart = match x.map(|i| columns[i].kind) {
        Some(Kind::Text) => plot::Chart::Bar,
        _ => plot::Chart::Line,
    };
    Suggestion { chart, x, series }
}

fn summarize(cache: &Cache, table: Table, file: Option<String>) -> Summary {
    let columns = table
        .columns
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let typed = c.kind != Kind::Text;
            let (min, max) = c.values.iter().flatten().fold(
                (None, None),
                |(lo, hi): (Option<f64>, Option<f64>), v| {
                    (
                        Some(lo.map_or(*v, |l| l.min(*v))),
                        Some(hi.map_or(*v, |h| h.max(*v))),
                    )
                },
            );
            ColumnInfo {
                index,
                name: c.name.clone(),
                kind: c.kind,
                present: c.raw.iter().filter(|r| !infer::is_missing(r)).count(),
                min: min.filter(|_| typed),
                max: max.filter(|_| typed),
            }
        })
        .collect();
    let preview = (0..table.rows.min(PREVIEW_ROWS))
        .map(|row| {
            table
                .columns
                .iter()
                .map(|c| truncate(&c.raw[row]))
                .collect()
        })
        .collect();
    let suggestion = suggest(&table);
    let (format, delimiter, header, json_path, rows, notes) = (
        table.format,
        table.delimiter,
        table.header,
        table.json_path.clone(),
        table.rows,
        table.notes.clone(),
    );
    let token = cache.store(table);
    Summary {
        token,
        format,
        delimiter,
        header,
        json_path,
        rows,
        columns,
        preview,
        notes,
        suggestion,
        file,
    }
}

/// 按扩展名决定格式（用户明确选择时以选择为准）
fn format_for(path: &str, format: Format) -> Format {
    if format != Format::Auto {
        return format;
    }
    let lower = path.to_lowercase();
    if lower.ends_with(".json") {
        Format::Json
    } else if lower.ends_with(".jsonl") || lower.ends_with(".ndjson") {
        Format::Jsonl
    } else if lower.ends_with(".tsv") || lower.ends_with(".tab") {
        Format::Tsv
    } else {
        Format::Auto
    }
}

fn read_file(path: &str, ctx: &dyn TaskContext) -> PluginResult<String> {
    let size = ctx.file_size(path)?;
    if size > MAX_FILE {
        return Err(PluginError::new("plot.file_too_large").with("limit", "64 MB"));
    }
    let mut reader = ctx.open_file(path)?;
    let mut bytes = Vec::with_capacity(size as usize);
    let mut buf = vec![0u8; CHUNK];
    loop {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| PluginError::new("plot.read_failed").with("detail", e.to_string()))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        ctx.progress(bytes.len() as u64, size);
    }
    String::from_utf8(bytes).map_err(|_| PluginError::new("plot.not_utf8"))
}

fn save_png(args: SaveArgs) -> PluginResult<()> {
    let data = args
        .data
        .split_once(',')
        .map_or(args.data.as_str(), |(_, b64)| b64);
    let bytes = data_encoding::BASE64
        .decode(data.trim().as_bytes())
        .map_err(|_| PluginError::new("plot.png_invalid"))?;
    if bytes.len() > MAX_PNG || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(PluginError::new("plot.png_invalid"));
    }
    tf_plugin_guest::net::write_file(&args.path, &bytes)
}

pub struct QuickPlot {
    manifest: Manifest,
    cache: Cache,
}

impl Default for QuickPlot {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            cache: Cache::default(),
        }
    }
}

impl ToolPlugin for QuickPlot {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => {
                let args: ParseArgs = parse_args(args)?;
                let table = table::parse(&args.text, args.format, args.header)?;
                to_value(summarize(&self.cache, table, None))
            }
            "plot" => {
                let args: plot::PlotArgs = parse_args(args)?;
                let table = self.cache.get(args.token)?;
                to_value(plot::build(&table, &args)?)
            }
            "save_png" => {
                save_png(parse_args(args)?)?;
                Ok(Value::Null)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "load" => {
                let args: LoadArgs = parse_args(args)?;
                ctx.stage("plot.reading");
                let text = read_file(&args.path, ctx)?;
                ctx.stage("plot.parsing");
                let table = table::parse(&text, format_for(&args.path, args.format), args.header)?;
                ctx.log(
                    LogLevel::Info,
                    "plot.loaded",
                    json!({ "rows": table.rows, "columns": table.columns.len() }),
                );
                let name = std::path::Path::new(&args.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned());
                to_value(summarize(&self.cache, table, name))
            }
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(QuickPlot);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
