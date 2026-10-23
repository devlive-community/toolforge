//! INI 与 JSON 数据模型互转。
//!
//! 节名中的点号表示嵌套（`[server.tls]`），`key[] = v` 表示数组；注释以 `;` 或 `#` 开头。

use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

use crate::convert::Warning;
use crate::csvfmt::infer;

fn invalid(line: usize, detail: &str) -> PluginError {
    PluginError::new("format.ini_invalid")
        .with("line", line)
        .with("column", 1)
        .with("detail", detail.to_owned())
}

/// 去掉引号；未加引号时去掉行尾 ` ;` / ` #` 注释。返回值与是否带引号
fn unquote(raw: &str) -> (String, bool) {
    let raw = raw.trim();
    for quote in ['"', '\''] {
        if raw.len() >= 2
            && raw.starts_with(quote)
            && let Some(end) = raw[1..].find(quote)
        {
            let inner = &raw[1..1 + end];
            let text = if quote == '"' {
                inner.replace("\\\"", "\"").replace("\\\\", "\\")
            } else {
                inner.to_owned()
            };
            return (text, true);
        }
    }
    let mut end = raw.len();
    for (index, c) in raw.char_indices() {
        if (c == ';' || c == '#') && index > 0 && raw[..index].ends_with(char::is_whitespace) {
            end = index;
            break;
        }
    }
    (raw[..end].trim_end().to_owned(), false)
}

fn section<'a>(
    root: &'a mut Map<String, Value>,
    path: &[&str],
    line: usize,
) -> PluginResult<&'a mut Map<String, Value>> {
    let mut current = root;
    for part in path {
        let entry = current
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        current = entry
            .as_object_mut()
            .ok_or_else(|| invalid(line, &format!("[{}] conflicts with a key", path.join("."))))?;
    }
    Ok(current)
}

pub fn parse(input: &str, infer_types: bool) -> PluginResult<Value> {
    let mut root = Map::new();
    let mut path: Vec<String> = Vec::new();
    for (index, raw) in input.lines().enumerate() {
        let line = index + 1;
        let text = raw.trim();
        if text.is_empty() || text.starts_with(';') || text.starts_with('#') {
            continue;
        }
        if let Some(name) = text.strip_prefix('[') {
            let name = name
                .split_once(']')
                .map(|(name, _)| name.trim())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| invalid(line, "invalid section header"))?;
            path = name.split('.').map(|p| p.trim().to_owned()).collect();
            let parts: Vec<&str> = path.iter().map(String::as_str).collect();
            section(&mut root, &parts, line)?;
            continue;
        }
        let split = text
            .find(['=', ':'])
            .ok_or_else(|| invalid(line, "expected key = value"))?;
        let key = text[..split].trim();
        if key.is_empty() {
            return Err(invalid(line, "empty key"));
        }
        let (raw_value, quoted) = unquote(&text[split + 1..]);
        let value = if quoted || !infer_types {
            Value::String(raw_value)
        } else {
            infer(&raw_value)
        };
        let parts: Vec<&str> = path.iter().map(String::as_str).collect();
        let target = section(&mut root, &parts, line)?;
        if let Some(key) = key.strip_suffix("[]") {
            let key = key.trim_end().to_owned();
            match target
                .entry(key)
                .or_insert_with(|| Value::Array(Vec::new()))
            {
                Value::Array(items) => items.push(value),
                _ => return Err(invalid(line, "array key conflicts with a value")),
            }
        } else {
            target.insert(key.to_owned(), value);
        }
    }
    Ok(Value::Object(root))
}

/// 会被重新推断为其他类型或含特殊字符的字符串需要加引号
fn quote_string(text: &str) -> String {
    let needs = text.is_empty()
        || text != text.trim()
        || text.contains([';', '#', '"', '\n'])
        || !infer(text).is_string();
    if needs {
        format!(
            "\"{}\"",
            text.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        )
    } else {
        text.to_owned()
    }
}

fn scalar(value: &Value) -> Option<String> {
    Some(match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => quote_string(s),
        Value::Array(_) | Value::Object(_) => return None,
    })
}

fn write_section(
    out: &mut String,
    name: &str,
    map: &Map<String, Value>,
    warnings: &mut Vec<Warning>,
) {
    let path = |key: &str| {
        if name.is_empty() {
            format!("$.{key}")
        } else {
            format!("$.{name}.{key}")
        }
    };
    let mut nested: Vec<(String, &Map<String, Value>)> = Vec::new();
    let mut body = String::new();
    for (key, value) in map {
        if let Some(text) = scalar(value) {
            body.push_str(&format!("{key} = {text}\n"));
            continue;
        }
        let full = if name.is_empty() {
            key.clone()
        } else {
            format!("{name}.{key}")
        };
        match value {
            Value::Object(child) => nested.push((full, child)),
            Value::Array(items) => {
                if items.is_empty() {
                    warnings.push(Warning::new("empty_container", &path(key)));
                }
                for (index, item) in items.iter().enumerate() {
                    match (scalar(item), item) {
                        (Some(text), _) => body.push_str(&format!("{key}[] = {text}\n")),
                        (None, Value::Object(child)) => {
                            warnings.push(Warning::new("ini_array_objects", &path(key)));
                            nested.push((format!("{full}.{index}"), child));
                        }
                        _ => warnings.push(Warning::new("dropped_nested_array", &path(key))),
                    }
                }
            }
            _ => {}
        }
    }
    if !name.is_empty() && (!body.is_empty() || nested.is_empty()) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("[{name}]\n"));
    }
    out.push_str(&body);
    for (child_name, child) in nested {
        write_section(out, &child_name, child, warnings);
    }
}

pub fn write(value: &Value, warnings: &mut Vec<Warning>) -> PluginResult<String> {
    let Value::Object(map) = value else {
        return Err(PluginError::new("format.ini_root"));
    };
    let mut out = String::new();
    write_section(&mut out, "", map, warnings);
    warnings.dedup();
    Ok(out)
}

#[cfg(test)]
#[path = "inifmt_test.rs"]
mod tests;
