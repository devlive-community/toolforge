//! 按原格式写回：应用逐条编辑，并可把基准中缺失的键补进目标文件。
//!
//! - JSON / ARB：保留键顺序与缩进风格；缺失的键插在基准中前一个同级键之后（找不到时放在末尾）
//! - properties：按行修改，注释、空行与其他行原样保留；新增的键插在基准中前一个键所在行之后
//! - YAML：重新序列化会丢失注释与格式，不支持写回

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

use crate::compare::FileInput;
use crate::source::{self, Entry, Format};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Fill {
    /// 不补缺失的键
    #[default]
    None,
    /// 补空字符串
    Empty,
    /// 补基准文本（便于后续翻译时对照）
    Base,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub base: FileInput,
    pub target: FileInput,
    /// 键路径 → 新文本
    #[serde(default)]
    pub edits: BTreeMap<String, String>,
    #[serde(default)]
    pub fill: Fill,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub text: String,
    pub added: Vec<String>,
    pub updated: Vec<String>,
}

/// 要写入的一项：路径段、键与文本
struct Change {
    path: Vec<String>,
    key: String,
    value: String,
}

pub fn run(args: Args) -> PluginResult<Output> {
    let format = args.target.format()?;
    if format == Format::Yaml {
        return Err(PluginError::new("i18n.write_unsupported").with("format", "YAML"));
    }
    let base = args.base.entries()?;
    let target = args.target.entries()?;
    let target_keys: HashMap<&str, &Entry> = target.iter().map(|e| (e.key.as_str(), e)).collect();
    let base_keys: HashMap<&str, &Entry> = base.iter().map(|e| (e.key.as_str(), e)).collect();

    // 编辑：已有的键原位修改；基准里有但目标缺失的键按新增处理
    let mut updates = Vec::new();
    let mut additions: Vec<Change> = Vec::new();
    for (key, value) in &args.edits {
        if let Some(entry) = target_keys.get(key.as_str()) {
            if entry.value != *value {
                updates.push(Change {
                    path: entry.path.clone(),
                    key: key.clone(),
                    value: value.clone(),
                });
            }
        } else if let Some(entry) = base_keys.get(key.as_str()) {
            additions.push(Change {
                path: entry.path.clone(),
                key: key.clone(),
                value: value.clone(),
            });
        } else {
            return Err(PluginError::new("i18n.unknown_key").with("key", key.as_str()));
        }
    }
    if args.fill != Fill::None {
        for entry in &base {
            if target_keys.contains_key(entry.key.as_str())
                || args.edits.contains_key(&entry.key)
                || additions.iter().any(|c| c.key == entry.key)
            {
                continue;
            }
            additions.push(Change {
                path: entry.path.clone(),
                key: entry.key.clone(),
                value: if args.fill == Fill::Base {
                    entry.value.clone()
                } else {
                    String::new()
                },
            });
        }
    }
    // 新增按基准顺序排列，插入位置才能稳定
    let order: HashMap<&str, usize> = base
        .iter()
        .enumerate()
        .map(|(i, e)| (e.key.as_str(), i))
        .collect();
    additions.sort_by_key(|c| order.get(c.key.as_str()).copied().unwrap_or(usize::MAX));

    let text = match format {
        Format::Json | Format::Arb => write_json(&args.target, &base, &updates, &additions)?,
        Format::Properties => {
            write_properties(&args.target.text, &base, &target, &updates, &additions)
        }
        Format::Yaml => unreachable!("rejected above"),
    };
    Ok(Output {
        text,
        added: additions.into_iter().map(|c| c.key).collect(),
        updated: updates.into_iter().map(|c| c.key).collect(),
    })
}

/// 检测缩进：第一个以空白开头的非空行
fn detect_indent(text: &str) -> Vec<u8> {
    text.lines()
        .find(|l| l.starts_with([' ', '\t']) && !l.trim().is_empty())
        .map(|l| {
            l.bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .collect()
        })
        .unwrap_or_else(|| b"  ".to_vec())
}

fn set_json(root: &mut Value, path: &[String], value: &str, base_siblings: &[&[String]]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut node = root;
    for segment in parents {
        let next = match node {
            Value::Object(map) => {
                if !map.contains_key(segment) {
                    map.insert(segment.clone(), Value::Object(Map::new()));
                }
                map.get_mut(segment)
            }
            Value::Array(items) => segment.parse::<usize>().ok().and_then(|i| items.get_mut(i)),
            _ => None,
        };
        match next {
            Some(next) => node = next,
            None => return,
        }
    }
    match node {
        Value::Object(map) => {
            let new = Value::String(value.to_owned());
            if let Some(slot) = map.get_mut(last) {
                *slot = new;
                return;
            }
            // 插在基准中排在它前面、且目标里已有的最近一个同级键之后
            let position = base_siblings
                .iter()
                .rev()
                .filter_map(|p| p.last())
                .find_map(|sibling| map.keys().position(|k| k == sibling))
                .map_or(map.len(), |i| i + 1);
            map.shift_insert(position, last.clone(), new);
        }
        Value::Array(items) => {
            if let Some(slot) = last.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
                *slot = Value::String(value.to_owned());
            }
        }
        _ => {}
    }
}

fn write_json(
    target: &FileInput,
    base: &[Entry],
    updates: &[Change],
    additions: &[Change],
) -> PluginResult<String> {
    let mut root: Value = serde_json::from_str(&target.text).map_err(|e| {
        PluginError::new("i18n.parse_failed")
            .with("file", target.name.as_str())
            .with("detail", e.to_string())
    })?;
    for change in updates {
        set_json(&mut root, &change.path, &change.value, &[]);
    }
    for change in additions {
        // 基准中同一父级、排在它前面的键
        let parent = &change.path[..change.path.len() - 1];
        let position = base.iter().position(|e| e.path == change.path).unwrap_or(0);
        let siblings: Vec<&[String]> = base[..position]
            .iter()
            .filter(|e| e.path.len() > parent.len() && e.path[..parent.len()] == *parent)
            .map(|e| &e.path[..=parent.len()])
            .collect();
        set_json(&mut root, &change.path, &change.value, &siblings);
    }
    let indent = detect_indent(&target.text);
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent);
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
    serde::Serialize::serialize(&root, &mut serializer)
        .map_err(|e| PluginError::new("i18n.write_failed").with("detail", e.to_string()))?;
    let mut text = String::from_utf8(out).unwrap_or_default();
    if target.text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// properties 转义；ascii_only 时非 ASCII 字符写成 \uXXXX（原文件使用这种写法时）
fn escape_properties(text: &str, is_key: bool, ascii_only: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '=' | ':' | '#' | '!' if is_key => {
                out.push('\\');
                out.push(c);
            }
            ' ' if is_key || i == 0 => out.push_str("\\ "),
            c if ascii_only && !c.is_ascii() => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn write_properties(
    text: &str,
    base: &[Entry],
    target: &[Entry],
    updates: &[Change],
    additions: &[Change],
) -> String {
    let ascii_only = text.contains("\\u") && text.is_ascii();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    // 行号 → 替换后的内容（None 表示删除该行，用于多行续行的后续行）
    let mut replaced: HashMap<usize, Option<String>> = HashMap::new();
    for change in updates {
        // 重复的键以最后一次出现为准
        let Some(entry) = target.iter().rev().find(|e| e.key == change.key) else {
            continue;
        };
        let Some((start, end)) = entry.lines else {
            continue;
        };
        let first = &lines[start];
        let indent = &first[..first.len() - first.trim_start().len()];
        let (key_raw, _) = source::split_property(first.trim_start());
        let separator = first.trim_start()[key_raw.len()..]
            .chars()
            .take_while(|c| matches!(c, ' ' | '\t' | '=' | ':'))
            .collect::<String>();
        let separator = if separator.is_empty() {
            "=".to_owned()
        } else {
            separator
        };
        replaced.insert(
            start,
            Some(format!(
                "{indent}{key_raw}{separator}{}",
                escape_properties(&change.value, false, ascii_only)
            )),
        );
        for line in start + 1..end {
            replaced.insert(line, None);
        }
    }
    // 新增：插在基准中前一个、目标已有的键之后
    let mut after: HashMap<usize, Vec<String>> = HashMap::new();
    let mut tail = Vec::new();
    for change in additions {
        let line = format!(
            "{}={}",
            escape_properties(&change.key, true, ascii_only),
            escape_properties(&change.value, false, ascii_only)
        );
        let position = base.iter().position(|e| e.key == change.key).unwrap_or(0);
        let anchor = base[..position]
            .iter()
            .rev()
            .find_map(|e| target.iter().rev().find(|t| t.key == e.key))
            .and_then(|t| t.lines.map(|(_, end)| end - 1));
        match anchor {
            Some(line_index) => after.entry(line_index).or_default().push(line),
            None => tail.push(line),
        }
    }
    let mut out = String::with_capacity(text.len() + additions.len() * 32);
    for (i, line) in lines.drain(..).enumerate() {
        match replaced.remove(&i) {
            Some(Some(new)) => {
                out.push_str(&new);
                out.push('\n');
            }
            Some(None) => {}
            None => {
                out.push_str(&line);
                out.push('\n');
            }
        }
        if let Some(block) = after.remove(&i) {
            for added in block {
                out.push_str(&added);
                out.push('\n');
            }
        }
    }
    for added in tail {
        out.push_str(&added);
        out.push('\n');
    }
    if !text.ends_with('\n') && !text.is_empty() && additions.is_empty() {
        out.pop();
    }
    out
}

#[cfg(test)]
#[path = "write_test.rs"]
mod tests;
