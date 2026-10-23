//! 输出格式：JSON、JSON Lines、CSV 与 SQL INSERT（MySQL / PostgreSQL / SQLite）。

use serde::Deserialize;
use serde_json::Value;

use crate::generate::{Generated, object};

/// 每条 INSERT 语句包含的行数
const SQL_BATCH: usize = 100;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Json,
    Jsonl,
    Csv,
    Sql,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Dialect {
    #[default]
    Mysql,
    Postgres,
    Sqlite,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub format: Format,
    #[serde(default)]
    pub dialect: Dialect,
    #[serde(default = "default_table")]
    pub table: String,
}

fn default_table() -> String {
    "mock_data".into()
}

fn csv_cell(value: &Value) -> String {
    let text = match value {
        Value::Null => return String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

fn quote_identifier(name: &str, dialect: Dialect) -> String {
    match dialect {
        Dialect::Mysql => format!("`{}`", name.replace('`', "``")),
        _ => format!("\"{}\"", name.replace('"', "\"\"")),
    }
}

fn sql_value(value: &Value, dialect: Dialect) -> String {
    match value {
        Value::Null => "NULL".into(),
        Value::Bool(b) => match (dialect, b) {
            (Dialect::Postgres, true) => "TRUE".into(),
            (Dialect::Postgres, false) => "FALSE".into(),
            (_, true) => "1".into(),
            (_, false) => "0".into(),
        },
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let escaped = s.replace('\'', "''");
            // MySQL 默认把反斜杠当作转义符
            if dialect == Dialect::Mysql {
                format!("'{}'", escaped.replace('\\', "\\\\"))
            } else {
                format!("'{escaped}'")
            }
        }
        other => format!("'{}'", other.to_string().replace('\'', "''")),
    }
}

pub fn render(data: &Generated, options: &Options) -> String {
    let columns = &data.columns;
    match options.format {
        Format::Json => {
            let rows: Vec<Value> = data
                .rows
                .iter()
                .map(|row| Value::Object(object(columns, row)))
                .collect();
            serde_json::to_string_pretty(&rows).unwrap_or_default() + "\n"
        }
        Format::Jsonl => data
            .rows
            .iter()
            .map(|row| {
                serde_json::to_string(&Value::Object(object(columns, row))).unwrap_or_default()
                    + "\n"
            })
            .collect(),
        Format::Csv => {
            let mut out = columns
                .iter()
                .map(|c| csv_cell(&Value::String(c.clone())))
                .collect::<Vec<_>>()
                .join(",");
            out.push('\n');
            for row in &data.rows {
                out.push_str(&row.iter().map(csv_cell).collect::<Vec<_>>().join(","));
                out.push('\n');
            }
            out
        }
        Format::Sql => {
            let table = quote_identifier(options.table.trim(), options.dialect);
            let names = columns
                .iter()
                .map(|c| quote_identifier(c, options.dialect))
                .collect::<Vec<_>>()
                .join(", ");
            let mut out = String::new();
            for batch in data.rows.chunks(SQL_BATCH) {
                out.push_str(&format!("INSERT INTO {table} ({names}) VALUES\n"));
                let values: Vec<String> = batch
                    .iter()
                    .map(|row| {
                        let cells: Vec<String> =
                            row.iter().map(|v| sql_value(v, options.dialect)).collect();
                        format!("  ({})", cells.join(", "))
                    })
                    .collect();
                out.push_str(&values.join(",\n"));
                out.push_str(";\n");
            }
            out
        }
    }
}

#[cfg(test)]
#[path = "output_test.rs"]
mod tests;
