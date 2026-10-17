//! 把粘贴的文本或文件解析为表格：自动识别 CSV / TSV / 分号 / 竖线 / 空白分隔、JSON 数组与 JSON Lines，
//! 识别表头，推断每列是数值、时间还是文本。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::infer::{self, Kind};

/// 行数上限，超出部分丢弃并提示
pub const MAX_ROWS: usize = 200_000;
/// 列数上限
pub const MAX_COLUMNS: usize = 256;
/// 嵌套对象展开的最大深度
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Auto,
    Csv,
    Tsv,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum HeaderMode {
    #[default]
    Auto,
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Delimiter {
    Comma,
    Tab,
    Semicolon,
    Pipe,
    Whitespace,
}

impl Delimiter {
    fn byte(self) -> Option<u8> {
        match self {
            Self::Comma => Some(b','),
            Self::Tab => Some(b'\t'),
            Self::Semicolon => Some(b';'),
            Self::Pipe => Some(b'|'),
            Self::Whitespace => None,
        }
    }
}

/// 解析时的提示（不阻止出图）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    pub code: &'static str,
    pub params: Value,
}

fn note(code: &'static str, params: Value) -> Note {
    Note { code, params }
}

pub struct Column {
    pub name: String,
    pub kind: Kind,
    /// 原始文本（空值为空字符串）
    pub raw: Vec<String>,
    /// 数值列为数值，时间列为毫秒时间戳；无法解析的为 None
    pub values: Vec<Option<f64>>,
}

pub struct Table {
    pub format: Format,
    pub delimiter: Option<Delimiter>,
    pub header: bool,
    /// JSON 中数据数组所在路径（根即数组时为空）
    pub json_path: Option<String>,
    pub rows: usize,
    pub columns: Vec<Column>,
    pub notes: Vec<Note>,
}

fn empty_input() -> PluginError {
    PluginError::new("plot.empty_input")
}

/// 解析入口
pub fn parse(text: &str, format: Format, header: HeaderMode) -> PluginResult<Table> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Err(empty_input());
    }
    let format = match format {
        Format::Auto => detect(text),
        other => other,
    };
    let mut notes = Vec::new();
    let (names, rows, delimiter, header, json_path) = match format {
        Format::Json => {
            let ((names, rows), path) = json_rows(text, &mut notes)?;
            (names, rows, None, true, path)
        }
        Format::Jsonl => {
            let (names, rows) = jsonl_rows(text, &mut notes)?;
            (names, rows, None, true, None)
        }
        Format::Csv | Format::Tsv | Format::Auto => {
            let delimiter = if format == Format::Tsv {
                Delimiter::Tab
            } else {
                sniff(text)
            };
            let records = split_records(text, delimiter)?;
            let (names, rows, header) = apply_header(records, header);
            (names, rows, Some(delimiter), header, None)
        }
    };
    let format = if format == Format::Csv && delimiter == Some(Delimiter::Tab) {
        Format::Tsv
    } else {
        format
    };
    build(format, delimiter, header, json_path, names, rows, notes)
}

/// 首个非空字符是 [ 或 { 时按 JSON；每行都是对象时按 JSON Lines
fn detect(text: &str) -> Format {
    let trimmed = text.trim_start();
    if trimmed.starts_with('[') {
        return Format::Json;
    }
    if trimmed.starts_with('{') {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let jsonl = lines
            .by_ref()
            .take(5)
            .all(|l| l.starts_with('{') && serde_json::from_str::<Value>(l).is_ok());
        let more = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .nth(1)
            .is_some();
        return if jsonl && more {
            Format::Jsonl
        } else {
            Format::Json
        };
    }
    Format::Csv
}

// ---------- 分隔文本 ----------

fn csv_reader(text: &str, delimiter: u8) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes())
}

/// 前 50 行里列数一致且大于 1 的分隔符；都不合适时退回空白分隔，最后是逗号
fn sniff(text: &str) -> Delimiter {
    let sample: String = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(50)
        .collect::<Vec<_>>()
        .join("\n");
    let mut best: Option<(Delimiter, usize, usize)> = None;
    for delimiter in [
        Delimiter::Tab,
        Delimiter::Comma,
        Delimiter::Semicolon,
        Delimiter::Pipe,
    ] {
        let counts: Vec<usize> = csv_reader(&sample, delimiter.byte().expect("有分隔字符"))
            .records()
            .filter_map(Result::ok)
            .map(|r| r.len())
            .collect();
        let Some(score) = consistency(&counts) else {
            continue;
        };
        if best.is_none_or(|(_, s, f)| score.0 > s || (score.0 == s && score.1 > f)) {
            best = Some((delimiter, score.0, score.1));
        }
    }
    if let Some((delimiter, _, _)) = best {
        return delimiter;
    }
    let counts: Vec<usize> = sample
        .lines()
        .map(|l| l.split_whitespace().count())
        .collect();
    if consistency(&counts).is_some() {
        return Delimiter::Whitespace;
    }
    Delimiter::Comma
}

/// 出现最多的列数（需大于 1）占比超过 80% 时返回（行数, 列数）
fn consistency(counts: &[usize]) -> Option<(usize, usize)> {
    let mut freq = std::collections::BTreeMap::new();
    for &c in counts.iter().filter(|c| **c > 1) {
        *freq.entry(c).or_insert(0usize) += 1;
    }
    let (&fields, &lines) = freq
        .iter()
        .max_by_key(|(fields, lines)| (**lines, **fields))?;
    (lines * 5 >= counts.len() * 4).then_some((lines, fields))
}

fn split_records(text: &str, delimiter: Delimiter) -> PluginResult<Vec<Vec<String>>> {
    let mut records = Vec::new();
    match delimiter.byte() {
        Some(byte) => {
            for record in csv_reader(text, byte).records() {
                let record = record.map_err(|e| {
                    PluginError::new("plot.csv_invalid")
                        .with("line", e.position().map_or(0, |p| p.line()))
                })?;
                if record.iter().all(str::is_empty) {
                    continue;
                }
                records.push(record.iter().map(str::to_owned).collect());
                // 多读一行（可能是表头）再多一行，才能判断是否超出上限
                if records.len() > MAX_ROWS + 1 {
                    break;
                }
            }
        }
        None => {
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                records.push(line.split_whitespace().map(str::to_owned).collect());
                if records.len() > MAX_ROWS + 1 {
                    break;
                }
            }
        }
    }
    if records.is_empty() {
        return Err(empty_input());
    }
    Ok(records)
}

/// 自动：首行没有数值 / 时间单元格，或首行某列是文字而下面多为数值时，视为表头
fn looks_like_header(records: &[Vec<String>]) -> bool {
    let first = &records[0];
    let typed = |cell: &str| infer::number(cell).is_some() || infer::datetime(cell).is_some();
    if first.iter().all(|c| !typed(c)) {
        return true;
    }
    let body = &records[1..records.len().min(21)];
    first.iter().enumerate().any(|(i, cell)| {
        !cell.is_empty()
            && !typed(cell)
            && body.len() >= 2
            && body
                .iter()
                .filter(|r| r.get(i).is_some_and(|c| infer::number(c).is_some()))
                .count()
                * 2
                > body.len()
    })
}

fn apply_header(
    mut records: Vec<Vec<String>>,
    mode: HeaderMode,
) -> (Vec<String>, Vec<Vec<String>>, bool) {
    let header = match mode {
        HeaderMode::Yes => true,
        HeaderMode::No => false,
        HeaderMode::Auto => records.len() > 1 && looks_like_header(&records),
    };
    let width = records.iter().map(Vec::len).max().unwrap_or(0);
    let names = if header {
        let first = records.remove(0);
        (0..width)
            .map(|i| {
                first
                    .get(i)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("#{}", i + 1))
            })
            .collect()
    } else {
        (0..width).map(|i| format!("#{}", i + 1)).collect()
    };
    (names, records, header)
}

// ---------- JSON ----------

/// 根是数组时直接使用；根是对象时找到第一个（广度优先，最深两层）非空数组
fn find_array(root: &Value) -> Option<(&Vec<Value>, Option<String>)> {
    if let Value::Array(items) = root {
        return Some((items, None));
    }
    let mut queue = vec![(root, String::new())];
    for _ in 0..2 {
        let mut next = Vec::new();
        for (value, path) in queue {
            let Value::Object(map) = value else { continue };
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match child {
                    Value::Array(items) if !items.is_empty() => {
                        return Some((items, Some(child_path)));
                    }
                    Value::Object(_) => next.push((child, child_path)),
                    _ => {}
                }
            }
        }
        queue = next;
    }
    None
}

/// 列名、各行单元格
type Rows = (Vec<String>, Vec<Vec<String>>);

fn json_rows(text: &str, notes: &mut Vec<Note>) -> PluginResult<(Rows, Option<String>)> {
    let root: Value = serde_json::from_str(text).map_err(|e| {
        PluginError::new("plot.json_invalid")
            .with("line", e.line())
            .with("column", e.column())
    })?;
    let (items, path) = find_array(&root).ok_or_else(|| PluginError::new("plot.json_no_array"))?;
    if let Some(path) = &path {
        notes.push(note("plot.json_path", serde_json::json!({ "path": path })));
    }
    let (names, rows) = flatten_items(items.iter(), notes);
    Ok(((names, rows), path))
}

fn jsonl_rows(text: &str, notes: &mut Vec<Note>) -> PluginResult<(Vec<String>, Vec<Vec<String>>)> {
    let mut items = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|e| {
            PluginError::new("plot.jsonl_invalid")
                .with("line", index + 1)
                .with("column", e.column())
        })?;
        items.push(value);
        if items.len() > MAX_ROWS {
            break;
        }
    }
    if items.is_empty() {
        return Err(empty_input());
    }
    Ok(flatten_items(items.iter(), notes))
}

/// 对象数组按点路径展开为列（列按首次出现的顺序）；数组的数组按位置成列；标量数组成为一列 value
fn flatten_items<'a>(
    items: impl Iterator<Item = &'a Value>,
    notes: &mut Vec<Note>,
) -> (Vec<String>, Vec<Vec<String>>) {
    let mut names: Vec<String> = Vec::new();
    let mut index = std::collections::HashMap::new();
    let mut rows: Vec<Vec<(usize, String)>> = Vec::new();
    let mut skipped_columns = 0usize;
    for item in items.take(MAX_ROWS + 1) {
        let mut cells = Vec::new();
        let mut flat = Vec::new();
        match item {
            Value::Object(_) => flatten("", item, 0, &mut flat),
            Value::Array(values) => {
                for (i, v) in values.iter().enumerate() {
                    flatten(&format!("#{}", i + 1), v, 1, &mut flat);
                }
            }
            other => flatten("value", other, 1, &mut flat),
        }
        for (name, cell) in flat {
            let column = match index.get(&name) {
                Some(&c) => c,
                None if names.len() < MAX_COLUMNS => {
                    index.insert(name.clone(), names.len());
                    names.push(name);
                    names.len() - 1
                }
                None => {
                    skipped_columns += 1;
                    continue;
                }
            };
            cells.push((column, cell));
        }
        rows.push(cells);
    }
    if skipped_columns > 0 {
        notes.push(note(
            "plot.too_many_columns",
            serde_json::json!({ "max": MAX_COLUMNS }),
        ));
    }
    let width = names.len();
    let rows = rows
        .into_iter()
        .map(|cells| {
            let mut row = vec![String::new(); width];
            for (column, cell) in cells {
                row[column] = cell;
            }
            row
        })
        .collect();
    (names, rows)
}

fn flatten(prefix: &str, value: &Value, depth: usize, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) if depth < MAX_DEPTH => {
            for (key, child) in map {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&name, child, depth + 1, out);
            }
        }
        Value::Null => out.push((prefix.to_owned(), String::new())),
        Value::String(s) => out.push((prefix.to_owned(), s.clone())),
        Value::Number(n) => out.push((prefix.to_owned(), n.to_string())),
        Value::Bool(b) => out.push((prefix.to_owned(), b.to_string())),
        // 数组与过深的对象保留为 JSON 文本
        other => out.push((prefix.to_owned(), other.to_string())),
    }
}

// ---------- 组装 ----------

fn build(
    format: Format,
    delimiter: Option<Delimiter>,
    header: bool,
    json_path: Option<String>,
    names: Vec<String>,
    mut rows: Vec<Vec<String>>,
    mut notes: Vec<Note>,
) -> PluginResult<Table> {
    if rows.len() > MAX_ROWS {
        rows.truncate(MAX_ROWS);
        notes.push(note(
            "plot.too_many_rows",
            serde_json::json!({ "max": MAX_ROWS }),
        ));
    }
    if rows.is_empty() || names.is_empty() {
        return Err(PluginError::new("plot.no_rows"));
    }
    let mut names = names;
    if names.len() > MAX_COLUMNS {
        names.truncate(MAX_COLUMNS);
        notes.push(note(
            "plot.too_many_columns",
            serde_json::json!({ "max": MAX_COLUMNS }),
        ));
    }
    let ragged = rows.iter().filter(|r| r.len() != names.len()).count();
    if ragged > 0 && delimiter.is_some() {
        notes.push(note(
            "plot.ragged_rows",
            serde_json::json!({ "count": ragged }),
        ));
    }
    let mut columns: Vec<Column> = names
        .into_iter()
        .map(|name| Column {
            name,
            kind: Kind::Text,
            raw: Vec::with_capacity(rows.len()),
            values: Vec::new(),
        })
        .collect();
    for row in rows.iter_mut() {
        row.resize(columns.len(), String::new());
        for (column, cell) in columns.iter_mut().zip(row.drain(..)) {
            column.raw.push(cell);
        }
    }
    for column in &mut columns {
        let (kind, values) = infer::column(&column.name, &column.raw);
        column.kind = kind;
        column.values = values;
    }
    Ok(Table {
        format,
        delimiter,
        header,
        json_path,
        rows: rows.len(),
        columns,
        notes,
    })
}

#[cfg(test)]
#[path = "table_test.rs"]
mod tests;
