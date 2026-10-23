use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::PluginResult;

use crate::parse::{self, Mode};

const MAX_ITEMS: usize = 2000;
const PREVIEW_CHARS: usize = 80;

#[derive(Deserialize)]
pub struct Args {
    left: String,
    right: String,
    #[serde(default)]
    mode: Mode,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Serialize)]
pub struct Item {
    pub path: String,
    pub change: Change,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub items: Vec<Item>,
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub truncated: bool,
    pub elapsed_ms: f64,
}

fn preview(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() <= PREVIEW_CHARS {
        return text;
    }
    let mut cut: String = text.chars().take(PREVIEW_CHARS).collect();
    cut.push('…');
    cut
}

fn key_path(parent: &str, key: &str) -> String {
    let simple = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !key.starts_with(|c: char| c.is_ascii_digit());
    if simple {
        format!("{parent}.{key}")
    } else {
        format!("{parent}[{}]", Value::String(key.to_owned()))
    }
}

impl Report {
    fn push(&mut self, path: String, change: Change, left: Option<&Value>, right: Option<&Value>) {
        match change {
            Change::Added => self.added += 1,
            Change::Removed => self.removed += 1,
            Change::Changed => self.changed += 1,
        }
        if self.items.len() >= MAX_ITEMS {
            self.truncated = true;
            return;
        }
        self.items.push(Item {
            path,
            change,
            left: left.map(preview),
            right: right.map(preview),
        });
    }

    fn compare(&mut self, path: &str, left: &Value, right: &Value) {
        match (left, right) {
            (Value::Object(a), Value::Object(b)) => {
                for (key, lv) in a {
                    let child = key_path(path, key);
                    match b.get(key) {
                        Some(rv) => self.compare(&child, lv, rv),
                        None => self.push(child, Change::Removed, Some(lv), None),
                    }
                }
                for (key, rv) in b.iter().filter(|(k, _)| !a.contains_key(*k)) {
                    self.push(key_path(path, key), Change::Added, None, Some(rv));
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                for index in 0..a.len().max(b.len()) {
                    let child = format!("{path}[{index}]");
                    match (a.get(index), b.get(index)) {
                        (Some(lv), Some(rv)) => self.compare(&child, lv, rv),
                        (Some(lv), None) => self.push(child, Change::Removed, Some(lv), None),
                        (None, Some(rv)) => self.push(child, Change::Added, None, Some(rv)),
                        (None, None) => {}
                    }
                }
            }
            (a, b) if a != b => self.push(path.to_owned(), Change::Changed, Some(a), Some(b)),
            _ => {}
        }
    }
}

/// 结构化对比两份 JSON；解析失败时错误参数带上 `side`（left / right）
pub fn run(args: Args) -> PluginResult<Report> {
    let start = std::time::Instant::now();
    let left = parse::parse(&args.left, args.mode).map_err(|e| e.with("side", "left"))?;
    let right = parse::parse(&args.right, args.mode).map_err(|e| e.with("side", "right"))?;
    let mut report = Report::default();
    report.compare("$", &left, &right);
    report.elapsed_ms = (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0;
    Ok(report)
}

#[cfg(test)]
#[path = "diff_test.rs"]
mod tests;
