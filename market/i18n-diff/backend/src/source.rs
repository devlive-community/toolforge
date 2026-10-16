//! 读取翻译文件并展平为有序的「键路径 → 文本」列表。
//!
//! JSON / ARB / YAML 的嵌套对象以 . 连接键路径，数组以下标作为一段；ARB 中以 @ 开头的元数据键忽略。
//! properties 按 Java 规则解析（续行、转义、= : 或空白分隔），并记录每个键所在的行，供写回使用。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Arb,
    Yaml,
    Properties,
}

impl Format {
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        let ext = lower.rsplit('.').next()?;
        match ext {
            "json" | "json5" => Some(Self::Json),
            "arb" => Some(Self::Arb),
            "yaml" | "yml" => Some(Self::Yaml),
            "properties" => Some(Self::Properties),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// 原始路径段，写回时用于定位（键本身可能含有 .）
    pub path: Vec<String>,
    pub key: String,
    pub value: String,
    /// properties 中该条目占用的行区间 [start, end)
    pub lines: Option<(usize, usize)>,
}

pub fn join_path(path: &[String]) -> String {
    path.join(".")
}

fn parse_error(name: &str, detail: impl ToString) -> PluginError {
    PluginError::new("i18n.parse_failed")
        .with("file", name)
        .with("detail", detail.to_string())
}

pub fn parse(name: &str, text: &str, format: Format) -> PluginResult<Vec<Entry>> {
    match format {
        Format::Json | Format::Arb => {
            let value: Value = serde_json::from_str(text).map_err(|e| parse_error(name, e))?;
            if !value.is_object() {
                return Err(PluginError::new("i18n.not_object").with("file", name));
            }
            let mut out = Vec::new();
            flatten_json(&value, &mut Vec::new(), format == Format::Arb, &mut out);
            Ok(out)
        }
        Format::Yaml => {
            let value: serde_yaml_ng::Value =
                serde_yaml_ng::from_str(text).map_err(|e| parse_error(name, e))?;
            let mut out = Vec::new();
            match value {
                serde_yaml_ng::Value::Mapping(_) => flatten_yaml(&value, &mut Vec::new(), &mut out),
                serde_yaml_ng::Value::Null => {}
                _ => return Err(PluginError::new("i18n.not_object").with("file", name)),
            }
            Ok(out)
        }
        Format::Properties => Ok(parse_properties(text)),
    }
}

fn scalar_json(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn flatten_json(value: &Value, path: &mut Vec<String>, arb: bool, out: &mut Vec<Entry>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                // ARB 元数据（@key、@@locale）不是翻译文本
                if arb && path.is_empty() && key.starts_with('@') {
                    continue;
                }
                path.push(key.clone());
                flatten_json(child, path, arb, out);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push(index.to_string());
                flatten_json(child, path, arb, out);
                path.pop();
            }
        }
        scalar => out.push(Entry {
            key: join_path(path),
            path: path.clone(),
            value: scalar_json(scalar),
            lines: None,
        }),
    }
}

fn yaml_key(key: &serde_yaml_ng::Value) -> String {
    match key {
        serde_yaml_ng::Value::String(s) => s.clone(),
        serde_yaml_ng::Value::Bool(b) => b.to_string(),
        serde_yaml_ng::Value::Number(n) => n.to_string(),
        serde_yaml_ng::Value::Null => "null".to_owned(),
        other => serde_yaml_ng::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    }
}

fn flatten_yaml(value: &serde_yaml_ng::Value, path: &mut Vec<String>, out: &mut Vec<Entry>) {
    use serde_yaml_ng::Value as Y;
    match value {
        Y::Mapping(map) => {
            for (key, child) in map {
                path.push(yaml_key(key));
                flatten_yaml(child, path, out);
                path.pop();
            }
        }
        Y::Sequence(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push(index.to_string());
                flatten_yaml(child, path, out);
                path.pop();
            }
        }
        Y::Tagged(tagged) => flatten_yaml(&tagged.value, path, out),
        scalar => out.push(Entry {
            key: join_path(path),
            path: path.clone(),
            value: match scalar {
                Y::String(s) => s.clone(),
                Y::Null => String::new(),
                Y::Bool(b) => b.to_string(),
                Y::Number(n) => n.to_string(),
                _ => String::new(),
            },
            lines: None,
        }),
    }
}

/// 行末是否有奇数个反斜杠（续行）
fn continues(line: &str) -> bool {
    line.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

/// properties 的逻辑行：(起始行, 结束行, 拼接后的内容)
fn logical_lines(text: &str) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let start = i;
        let first = lines[i].trim_start();
        if first.is_empty() || first.starts_with('#') || first.starts_with('!') {
            i += 1;
            continue;
        }
        let mut content = String::new();
        let mut current = first;
        loop {
            if continues(current) && i + 1 < lines.len() {
                content.push_str(&current[..current.len() - 1]);
                i += 1;
                current = lines[i].trim_start();
            } else {
                content.push_str(current);
                break;
            }
        }
        i += 1;
        out.push((start, i, content));
    }
    out
}

/// 解码 properties 转义：\uXXXX、\n、\t、\r、\f 以及转义的任意字符
pub fn unescape_properties(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.clone().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(decoded) if hex.len() == 4 => {
                        out.push(decoded);
                        for _ in 0..4 {
                            chars.next();
                        }
                    }
                    _ => out.push('u'),
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// 键与值的分界：第一个未转义的 = 或 :，或空白
pub fn split_property(content: &str) -> (&str, &str) {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'=' | b':' => {
                return (&content[..i], content[i + 1..].trim_start());
            }
            b' ' | b'\t' | b'\x0c' => {
                let key = &content[..i];
                let rest = content[i..].trim_start();
                let rest = rest.strip_prefix(['=', ':']).map_or(rest, str::trim_start);
                return (key, rest);
            }
            _ => i += 1,
        }
    }
    (content, "")
}

pub fn parse_properties(text: &str) -> Vec<Entry> {
    logical_lines(text)
        .into_iter()
        .map(|(start, end, content)| {
            let (key, value) = split_property(&content);
            let key = unescape_properties(key);
            Entry {
                path: vec![key.clone()],
                key,
                value: unescape_properties(value),
                lines: Some((start, end)),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
