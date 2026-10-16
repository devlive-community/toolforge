//! Java .properties 与 JSON 数据模型互转。
//!
//! 读取时按点号与 `[n]` 下标还原嵌套结构；写出时把嵌套结构展平为 `a.b[0].c` 形式的键。

use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

use crate::convert::Warning;
use crate::csvfmt::infer;

/// 合并续行（行尾奇数个反斜杠），返回 (起始行号, 逻辑行)
fn logical_lines(input: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (index, raw) in input.lines().enumerate() {
        let text = match &current {
            Some(_) => raw.trim_start(),
            None => {
                let trimmed = raw.trim_start();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                    continue;
                }
                trimmed
            }
        };
        let backslashes = text.len() - text.trim_end_matches('\\').len();
        let continued = backslashes % 2 == 1;
        let piece = if continued {
            &text[..text.len() - 1]
        } else {
            text
        };
        let entry = current.get_or_insert_with(|| (index + 1, String::new()));
        entry.1.push_str(piece);
        if !continued {
            lines.push(current.take().expect("current line"));
        }
    }
    lines.extend(current);
    lines
}

fn unescape(text: &str, line: usize) -> PluginResult<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                let ch = u32::from_str_radix(&hex, 16)
                    .ok()
                    .filter(|_| hex.len() == 4)
                    .and_then(char::from_u32)
                    .ok_or_else(|| {
                        PluginError::new("format.properties_invalid")
                            .with("line", line)
                            .with("column", 1)
                            .with("detail", format!("invalid escape \\u{hex}"))
                    })?;
                out.push(ch);
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    Ok(out)
}

/// 拆分 key 与 value：key 在第一个未转义的 `=`、`:` 或空白处结束
fn split_entry(line: &str) -> (&str, &str) {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'=' | b':' => return (&line[..index], line[index + 1..].trim_start()),
            b' ' | b'\t' | b'\x0c' => {
                let rest = line[index..].trim_start();
                let rest = rest
                    .strip_prefix('=')
                    .or_else(|| rest.strip_prefix(':'))
                    .map_or(rest, str::trim_start);
                return (&line[..index], rest);
            }
            _ => index += 1,
        }
    }
    (line, "")
}

enum Segment {
    Key(String),
    Index(usize),
}

fn segments(key: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    for part in key.split('.') {
        let (name, mut rest) = part.split_once('[').map_or((part, ""), |(n, r)| (n, r));
        out.push(Segment::Key(name.to_owned()));
        while !rest.is_empty() {
            match rest
                .split_once(']')
                .and_then(|(n, r)| Some((n.parse().ok()?, r)))
            {
                Some((index, after)) => {
                    out.push(Segment::Index(index));
                    rest = after.strip_prefix('[').unwrap_or("");
                }
                None => return vec![Segment::Key(key.to_owned())],
            }
        }
    }
    if out
        .iter()
        .any(|s| matches!(s, Segment::Key(k) if k.is_empty()))
    {
        return vec![Segment::Key(key.to_owned())];
    }
    out
}

/// 沿路径插入，路径与已有值冲突时返回 false
fn insert(target: &mut Value, path: &[Segment], value: Value) -> bool {
    let Some((first, rest)) = path.split_first() else {
        if target.is_null() {
            *target = value;
            return true;
        }
        return false;
    };
    let slot = match first {
        Segment::Key(key) => {
            if target.is_null() {
                *target = Value::Object(Map::new());
            }
            let Value::Object(map) = target else {
                return false;
            };
            map.entry(key.clone()).or_insert(Value::Null)
        }
        Segment::Index(index) => {
            if target.is_null() {
                *target = Value::Array(Vec::new());
            }
            let Value::Array(items) = target else {
                return false;
            };
            // 下标过大视为冲突，避免 a[100000] 撑大数组
            if *index > items.len() + 1024 {
                return false;
            }
            if items.len() <= *index {
                items.resize(*index + 1, Value::Null);
            }
            &mut items[*index]
        }
    };
    insert(slot, rest, value)
}

pub fn parse(input: &str, infer_types: bool, warnings: &mut Vec<Warning>) -> PluginResult<Value> {
    let mut root = Value::Object(Map::new());
    for (line, text) in logical_lines(input) {
        let (raw_key, raw_value) = split_entry(&text);
        let key = unescape(raw_key, line)?;
        let text = unescape(raw_value, line)?;
        let value = if infer_types {
            infer(&text)
        } else {
            Value::String(text)
        };
        if !insert(&mut root, &segments(&key), value.clone()) {
            // 与已有层级冲突（如同时存在 a=1 与 a.b=2）时保留扁平键
            warnings.push(Warning::new("properties_conflict", &key));
            if let Value::Object(map) = &mut root {
                map.insert(key, value);
            }
        }
    }
    Ok(root)
}

fn escape(text: &str, key: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, c) in text.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '=' | ':' | '#' | '!' if key => {
                out.push('\\');
                out.push(c);
            }
            ' ' if key || index == 0 => out.push_str("\\ "),
            _ => out.push(c),
        }
    }
    out
}

fn flatten(prefix: &str, value: &Value, out: &mut String, warnings: &mut Vec<Warning>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() && !prefix.is_empty() {
                warnings.push(Warning::new("empty_container", &format!("$.{prefix}")));
            }
            for (key, item) in map {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&name, item, out, warnings);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                warnings.push(Warning::new("empty_container", &format!("$.{prefix}")));
            }
            for (index, item) in items.iter().enumerate() {
                flatten(&format!("{prefix}[{index}]"), item, out, warnings);
            }
        }
        scalar => {
            let text = match scalar {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            out.push_str(&escape(prefix, true));
            out.push('=');
            out.push_str(&escape(&text, false));
            out.push('\n');
        }
    }
}

pub fn write(value: &Value, warnings: &mut Vec<Warning>) -> PluginResult<String> {
    if !value.is_object() && !value.is_array() {
        return Err(PluginError::new("format.properties_root"));
    }
    let mut out = String::new();
    if value.is_array() {
        // 顶层数组写成 [0].a 不符合习惯，用 item 作为前缀
        flatten("item", value, &mut out, warnings);
    } else {
        flatten("", value, &mut out, warnings);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "propsfmt_test.rs"]
mod tests;
