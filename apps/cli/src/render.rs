//! 把插件结果渲染成终端文本。接口声明的 `output` 提示优先：
//! `{"text": 指针}` 直接打印该值，`{"table": 指针, "columns": [...]}` 打印表格；
//! 没有提示时按结构通用渲染：标量 `key: value`，对象数组为表格，嵌套对象缩进。

use serde_json::Value;

use crate::i18n::Messages;

/// 终端显示宽度：CJK 等全角字符按 2 列计
pub fn width(text: &str) -> usize {
    text.chars()
        .map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 })
        .sum()
}

/// 右侧补空格到指定显示宽度
pub fn pad(text: &str, to: usize) -> String {
    format!("{text}{}", " ".repeat(to.saturating_sub(width(text))))
}

fn cell(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(s) => s.clone(),
        Value::Array(items) if items.iter().all(|v| !v.is_object() && !v.is_array()) => {
            items.iter().map(cell).collect::<Vec<_>>().join(", ")
        }
        other => other.to_string(),
    }
}

/// 对齐的表格；单元格里的换行替换为空格，行尾不留空格
pub fn table(headers: &[String], rows: &[Vec<String>]) -> String {
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(|c| c.replace('\n', " ")).collect())
        .collect();
    let mut widths: Vec<usize> = headers.iter().map(|h| width(h)).collect();
    for row in &rows {
        for (i, value) in row.iter().enumerate().take(widths.len()) {
            widths[i] = widths[i].max(width(value));
        }
    }
    let line = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(i, value)| pad(value, widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_owned()
    };
    std::iter::once(line(headers))
        .chain(rows.iter().map(|row| line(row)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn object_table(items: &[Value], columns: &[(String, String)]) -> String {
    let headers: Vec<String> = columns.iter().map(|(_, label)| label.clone()).collect();
    let rows: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            columns
                .iter()
                .map(|(key, _)| cell(&item[key.as_str()]))
                .collect()
        })
        .collect();
    table(&headers, &rows)
}

/// 对象数组的列：按出现顺序合并所有键
fn auto_columns(items: &[Value]) -> Vec<(String, String)> {
    let mut keys: Vec<String> = Vec::new();
    for key in items
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys())
    {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    keys.into_iter().map(|k| (k.clone(), k)).collect()
}

fn is_rows(value: &Value) -> bool {
    matches!(value, Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_object))
}

fn generic(value: &Value, indent: usize, out: &mut Vec<String>) {
    let lead = " ".repeat(indent);
    let indented = |text: String, out: &mut Vec<String>| {
        out.extend(text.lines().map(|l| format!("{lead}{l}")));
    };
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match child {
                    Value::Object(inner) if !inner.is_empty() => {
                        out.push(format!("{lead}{key}:"));
                        generic(child, indent + 2, out);
                    }
                    Value::Array(items) if is_rows(child) => {
                        out.push(format!("{lead}{key}:"));
                        let rendered = object_table(items, &auto_columns(items));
                        out.extend(rendered.lines().map(|l| format!("{lead}  {l}")));
                    }
                    other => out.push(format!("{lead}{key}: {}", cell(other))),
                }
            }
        }
        Value::Array(items) if is_rows(value) => {
            indented(object_table(items, &auto_columns(items)), out);
        }
        Value::Array(items) => {
            for item in items {
                indented(cell(item), out);
            }
        }
        other => indented(cell(other), out),
    }
}

/// 渲染结果；结尾不带换行
pub fn render(result: &Value, output: Option<&Value>, messages: &Messages) -> String {
    if let Some(hint) = output {
        if let Some(value) = hint["text"].as_str().and_then(|p| result.pointer(p)) {
            return match value {
                Value::String(s) => s.trim_end_matches('\n').to_owned(),
                other => render(other, None, messages),
            };
        }
        if let Some(Value::Array(items)) = hint["table"].as_str().and_then(|p| result.pointer(p)) {
            let columns: Vec<(String, String)> = hint["columns"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    let key = c["key"].as_str().unwrap_or_default().to_owned();
                    let label = c["label"]
                        .as_str()
                        .map_or_else(|| key.clone(), |l| messages.manifest_text(l));
                    (key, label)
                })
                .collect();
            return object_table(items, &columns);
        }
    }
    let mut out = Vec::new();
    generic(result, 0, &mut out);
    out.join("\n")
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
