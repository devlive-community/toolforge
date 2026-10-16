//! 批量校验：多行文本或 CSV 指定列，逐条输出原值、规范化结果、问题与字段解释。

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{PluginError, PluginResult};

use crate::code::{self, Normalized, Report, Status};

pub const MAX_INPUT: usize = 10 * 1024 * 1024;
pub const MAX_ROWS: usize = 100_000;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Lines,
    Csv,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub input: String,
    #[serde(default)]
    pub mode: Mode,
    /// CSV 中的列：列名或 1 起始的序号
    #[serde(default)]
    pub column: String,
    #[serde(default = "comma")]
    pub delimiter: char,
}

fn comma() -> char {
    ','
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// 原文中的行号（CSV 为数据行号，表头为第 1 行）
    pub line: usize,
    pub raw: String,
    pub normalized: Normalized,
    #[serde(flatten)]
    pub report: Report,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub valid: usize,
    pub unconfirmed: usize,
    pub invalid: usize,
    pub empty: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub rules: &'static str,
    pub rows: Vec<Row>,
    pub summary: Summary,
    /// CSV 模式下可选的列名
    pub columns: Vec<String>,
}

/// (行号, 原值)
type Lines = Vec<(usize, String)>;

fn rows_from_lines(input: &str) -> Lines {
    input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_owned()))
        .collect()
}

fn rows_from_csv(args: &Args) -> PluginResult<(Lines, Vec<String>)> {
    if !args.delimiter.is_ascii() {
        return Err(PluginError::new("uscc.invalid_delimiter"));
    }
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(args.delimiter as u8)
        .flexible(true)
        .from_reader(args.input.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| PluginError::new("uscc.csv_invalid").with("detail", e.to_string()))?
        .iter()
        .map(str::to_owned)
        .collect();
    let wanted = args.column.trim();
    let index = if wanted.is_empty() {
        0
    } else if let Some(i) = headers.iter().position(|h| h.trim() == wanted) {
        i
    } else {
        match wanted.parse::<usize>() {
            Ok(n) if (1..=headers.len()).contains(&n) => n - 1,
            _ => return Err(PluginError::new("uscc.column_not_found").with("column", wanted)),
        }
    };
    let mut rows = Vec::new();
    for (i, record) in reader.records().enumerate() {
        let record = record
            .map_err(|e| PluginError::new("uscc.csv_invalid").with("detail", e.to_string()))?;
        rows.push((i + 2, record.get(index).unwrap_or_default().to_owned()));
    }
    Ok((rows, headers))
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.input.len() > MAX_INPUT {
        return Err(PluginError::new("uscc.too_large").with("mb", 10));
    }
    let (rows, columns) = match args.mode {
        Mode::Lines => (rows_from_lines(&args.input), Vec::new()),
        Mode::Csv => rows_from_csv(&args)?,
    };
    if rows.len() > MAX_ROWS {
        return Err(PluginError::new("uscc.too_many").with("max", MAX_ROWS));
    }
    let mut summary = Summary::default();
    let mut out = Vec::with_capacity(rows.len());
    for (line, raw) in rows {
        let normalized = code::normalize(&raw);
        if normalized.value.is_empty() {
            summary.empty += 1;
            continue;
        }
        let report = code::check(&normalized.value);
        match report.status {
            Status::Valid => summary.valid += 1,
            Status::Unconfirmed => summary.unconfirmed += 1,
            Status::Invalid => summary.invalid += 1,
        }
        out.push(Row {
            line,
            raw,
            normalized,
            report,
        });
    }
    Ok(Output {
        rules: code::RULES,
        rows: out,
        summary,
        columns,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    #[serde(flatten)]
    pub args: Args,
    pub format: ExportFormat,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// 导出报告：CSV 每行一条，问题以代码形式列出，便于再处理
pub fn export(args: ExportArgs) -> PluginResult<String> {
    let output = run(args.args)?;
    Ok(match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&json!({
            "rules": output.rules,
            "summary": output.summary,
            "rows": output.rows,
        }))
        .map_err(|e| PluginError::new("uscc.export_failed").with("detail", e.to_string()))?,
        ExportFormat::Csv => {
            let mut text = String::from(
                "line,raw,normalized,status,problems,department,category,region,expected_check\n",
            );
            for row in &output.rows {
                let problems: Vec<String> = row
                    .report
                    .problems
                    .iter()
                    .map(|p| match p {
                        code::Problem::Length { found } => format!("length:{found}"),
                        code::Problem::Char { position, found } => {
                            format!("char:{position}:{found}")
                        }
                        code::Problem::Check { expected, found } => {
                            format!("check:{found}->{expected}")
                        }
                    })
                    .collect();
                let fields = row.report.fields.as_ref();
                let expected = row.report.problems.iter().find_map(|p| match p {
                    code::Problem::Check { expected, .. } => Some(expected.as_str()),
                    _ => None,
                });
                let status = match row.report.status {
                    Status::Valid => "valid",
                    Status::Unconfirmed => "unconfirmed",
                    Status::Invalid => "invalid",
                };
                text.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{}\n",
                    row.line,
                    csv_field(&row.raw),
                    csv_field(&row.normalized.value),
                    status,
                    csv_field(&problems.join(";")),
                    fields.map_or("", |f| f.department.name.unwrap_or("")),
                    fields.map_or("", |f| f.category.name.unwrap_or("")),
                    fields.map_or(String::new(), |f| f.region.code.clone()),
                    expected.unwrap_or("")
                ));
            }
            text
        }
    })
}

#[cfg(test)]
#[path = "batch_test.rs"]
mod tests;
