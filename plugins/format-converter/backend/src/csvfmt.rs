//! CSV 与 JSON 数据模型之间的转换。

use serde::Deserialize;
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn byte(self) -> u8 {
        match self {
            Delimiter::Comma => b',',
            Delimiter::Semicolon => b';',
            Delimiter::Tab => b'\t',
            Delimiter::Pipe => b'|',
        }
    }
}

/// 推断单元格类型：整数、浮点数、布尔、空值，其余为字符串
fn infer(cell: &str) -> Value {
    let trimmed = cell.trim();
    match trimmed {
        "" => Value::Null,
        "true" | "TRUE" | "True" => Value::Bool(true),
        "false" | "FALSE" | "False" => Value::Bool(false),
        _ => {
            // 前导零的数字（邮编、编号）保持字符串
            let leading_zero =
                trimmed.len() > 1 && trimmed.starts_with('0') && !trimmed.starts_with("0.");
            if !leading_zero {
                if let Ok(i) = trimmed.parse::<i64>() {
                    return Value::Number(i.into());
                }
                if let Ok(f) = trimmed.parse::<f64>()
                    && f.is_finite()
                    && let Some(n) = serde_json::Number::from_f64(f)
                {
                    return Value::Number(n);
                }
            }
            Value::String(cell.to_owned())
        }
    }
}

pub fn parse(
    input: &str,
    delimiter: Delimiter,
    header: bool,
    infer_types: bool,
) -> PluginResult<Value> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter.byte())
        .has_headers(header)
        .flexible(true)
        .from_reader(input.as_bytes());
    let headers: Vec<String> = if header {
        reader
            .headers()
            .map_err(csv_error)?
            .iter()
            .enumerate()
            .map(|(i, h)| {
                if h.trim().is_empty() {
                    format!("column{}", i + 1)
                } else {
                    h.to_owned()
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let cell = |s: &str| {
        if infer_types {
            infer(s)
        } else {
            Value::String(s.to_owned())
        }
    };

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(csv_error)?;
        if header {
            let mut object = Map::new();
            for (index, value) in record.iter().enumerate() {
                let key = headers
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("column{}", index + 1));
                object.insert(key, cell(value));
            }
            rows.push(Value::Object(object));
        } else {
            rows.push(Value::Array(record.iter().map(cell).collect()));
        }
    }
    Ok(Value::Array(rows))
}

fn csv_error(err: csv::Error) -> PluginError {
    let mut error = PluginError::new("format.csv_invalid").with("detail", err.to_string());
    if let Some(position) = err.position() {
        error = error.with("line", position.line());
    }
    error
}

/// 嵌套对象展开为 a.b 列，数组与其他复杂值写为 JSON 文本
fn flatten(prefix: &str, value: &Value, out: &mut Map<String, Value>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, item) in map {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&name, item, out);
            }
        }
        other => {
            out.insert(prefix.to_owned(), other.clone());
        }
    }
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

pub fn write(value: &Value, delimiter: Delimiter) -> PluginResult<(String, usize)> {
    let rows = match value {
        Value::Array(rows) => rows.as_slice(),
        Value::Object(_) => std::slice::from_ref(value),
        _ => return Err(PluginError::new("format.csv_shape")),
    };
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter.byte())
        .from_writer(Vec::new());
    let error =
        |e: csv::Error| PluginError::new("format.csv_invalid").with("detail", e.to_string());

    if rows.iter().all(Value::is_array) {
        for row in rows {
            let cells: Vec<String> = row
                .as_array()
                .expect("array")
                .iter()
                .map(cell_text)
                .collect();
            writer.write_record(&cells).map_err(error)?;
        }
    } else {
        if !rows.iter().all(Value::is_object) {
            return Err(PluginError::new("format.csv_shape"));
        }
        let flat: Vec<Map<String, Value>> = rows
            .iter()
            .map(|row| {
                let mut out = Map::new();
                flatten("", row, &mut out);
                out
            })
            .collect();
        // 表头为所有行键的并集，保持首次出现的顺序
        let mut headers: Vec<String> = Vec::new();
        for row in &flat {
            for key in row.keys() {
                if !headers.contains(key) {
                    headers.push(key.clone());
                }
            }
        }
        writer.write_record(&headers).map_err(error)?;
        for row in &flat {
            let cells: Vec<String> = headers
                .iter()
                .map(|h| row.get(h).map(cell_text).unwrap_or_default())
                .collect();
            writer.write_record(&cells).map_err(error)?;
        }
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| PluginError::new("format.csv_invalid").with("detail", e.to_string()))?;
    let text = String::from_utf8(bytes)
        .map_err(|e| PluginError::new("format.csv_invalid").with("detail", e.to_string()))?;
    Ok((text, rows.len()))
}
