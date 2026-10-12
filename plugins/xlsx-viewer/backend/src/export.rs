//! 按当前视图（排序与筛选后）导出为 CSV / TSV / JSON / Markdown。

use std::io::Write;

use serde::Deserialize;
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult, cancelled};

use crate::table::{Kind, Table, parse_number};
use crate::view::{row_at, view_len};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Csv,
    Tsv,
    Json,
    Markdown,
}

fn io(err: impl std::fmt::Display) -> PluginError {
    PluginError::new("fs.io").with("detail", err.to_string())
}

/// JSON 的键：没有表头时用 column1…，重名时加序号
pub fn keys(table: &Table) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for (i, column) in table.columns.iter().enumerate() {
        let base = column
            .name
            .clone()
            .unwrap_or_else(|| format!("column{}", i + 1));
        let mut key = base.clone();
        let mut n = 2;
        while keys.contains(&key) {
            key = format!("{base}_{n}");
            n += 1;
        }
        keys.push(key);
    }
    keys
}

fn json_value(kind: Kind, cell: &str) -> Value {
    let trimmed = cell.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    match kind {
        Kind::Integer => trimmed
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::from(cell)),
        Kind::Float => parse_number(trimmed)
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::from(cell)),
        Kind::Boolean => {
            Value::Bool(trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("yes"))
        }
        _ => Value::from(cell),
    }
}

fn markdown_cell(cell: &str) -> String {
    cell.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace('\n', "<br>")
}

/// 写出视图中的行，每 10000 行回调一次进度（已写行数）
pub fn write(
    table: &Table,
    view: Option<&[u32]>,
    format: Format,
    has_header: bool,
    out: &mut impl Write,
    mut progress: impl FnMut(usize),
    is_cancelled: impl Fn() -> bool,
) -> PluginResult<usize> {
    let total = view_len(table, view);
    let width = table.width();
    let tick = |i: usize, progress: &mut dyn FnMut(usize)| -> PluginResult<()> {
        if i.is_multiple_of(10_000) {
            if is_cancelled() {
                return Err(cancelled());
            }
            progress(i);
        }
        Ok(())
    };
    match format {
        Format::Csv | Format::Tsv => {
            let mut writer = csv::WriterBuilder::new()
                .delimiter(if format == Format::Tsv { b'\t' } else { b',' })
                .from_writer(&mut *out);
            if has_header {
                writer
                    .write_record(
                        table
                            .columns
                            .iter()
                            .map(|c| c.name.as_deref().unwrap_or("")),
                    )
                    .map_err(io)?;
            }
            for i in 0..total {
                tick(i, &mut progress)?;
                let row = row_at(view, i);
                writer
                    .write_record((0..width).map(|c| table.cell(row, c)))
                    .map_err(io)?;
            }
            writer.flush().map_err(io)?;
        }
        Format::Json => {
            let keys = keys(table);
            out.write_all(b"[").map_err(io)?;
            for i in 0..total {
                tick(i, &mut progress)?;
                let row = row_at(view, i);
                let object: Map<String, Value> = keys
                    .iter()
                    .enumerate()
                    .map(|(c, key)| {
                        (
                            key.clone(),
                            json_value(table.columns[c].kind, table.cell(row, c)),
                        )
                    })
                    .collect();
                let separator: &[u8] = if i == 0 { b"\n  " } else { b",\n  " };
                out.write_all(separator).map_err(io)?;
                serde_json::to_writer(&mut *out, &object).map_err(io)?;
            }
            out.write_all(if total == 0 { b"]\n" } else { b"\n]\n" })
                .map_err(io)?;
        }
        Format::Markdown => {
            let header: Vec<String> = keys(table).iter().map(|k| markdown_cell(k)).collect();
            writeln!(out, "| {} |", header.join(" | ")).map_err(io)?;
            let align: Vec<&str> = table
                .columns
                .iter()
                .map(|c| if c.kind.is_numeric() { "---:" } else { "---" })
                .collect();
            writeln!(out, "| {} |", align.join(" | ")).map_err(io)?;
            for i in 0..total {
                tick(i, &mut progress)?;
                let row = row_at(view, i);
                let cells: Vec<String> = (0..width)
                    .map(|c| markdown_cell(table.cell(row, c)))
                    .collect();
                writeln!(out, "| {} |", cells.join(" | ")).map_err(io)?;
            }
        }
    }
    progress(total);
    Ok(total)
}

#[cfg(test)]
#[path = "export_test.rs"]
mod tests;
