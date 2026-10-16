//! 多语言文件对比（第一个文件为基准）：
//! - 缺失：基准有、该文件没有的键
//! - 多余：基准没有、该文件有的键
//! - 空值：基准有文本、该文件为空
//! - 疑似未翻译：与基准文本完全相同（基准文本中含字母，且不只是占位符）
//! - 占位符不一致：两边的占位符集合（不带位置的 printf 占位符还要求次数）不同
//! - 重复：同一文件中出现多次的键（properties 等格式允许重复，后者生效）

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::placeholders::{self, Placeholders};
use crate::source::{self, Entry, Format};

pub const MAX_FILES: usize = 20;
pub const MAX_FILE: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInput {
    pub name: String,
    pub text: String,
    /// 为空时按扩展名识别
    #[serde(default)]
    pub format: Option<Format>,
}

impl FileInput {
    pub fn format(&self) -> PluginResult<Format> {
        self.format
            .or_else(|| Format::from_name(&self.name))
            .ok_or_else(|| PluginError::new("i18n.unknown_format").with("file", self.name.as_str()))
    }

    pub fn entries(&self) -> PluginResult<Vec<Entry>> {
        if self.text.len() > MAX_FILE {
            return Err(PluginError::new("i18n.too_large")
                .with("file", self.name.as_str())
                .with("mb", MAX_FILE / 1024 / 1024));
        }
        source::parse(&self.name, &self.text, self.format()?)
    }
}

#[derive(Debug, Deserialize)]
pub struct Args {
    pub files: Vec<FileInput>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Issue {
    Missing,
    Extra,
    Empty,
    Untranslated,
    Placeholders,
    Duplicate,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub value: Option<String>,
    pub issues: Vec<Issue>,
    /// 相对基准缺少 / 多出的占位符
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_placeholders: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_placeholders: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub key: String,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub name: String,
    pub format: Option<Format>,
    pub keys: usize,
    pub missing: usize,
    pub extra: usize,
    pub empty: usize,
    pub untranslated: usize,
    pub placeholders: usize,
    pub duplicate: usize,
    /// 基准中的键已有非空译文的比例（0–100）
    pub coverage: f64,
    /// 可以按原格式写回
    pub writable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub files: Vec<Summary>,
    pub rows: Vec<Row>,
}

/// 一个文件的键索引：首次出现顺序、出现次数与最后一次的值
struct Index {
    order: Vec<String>,
    count: HashMap<String, usize>,
    value: HashMap<String, String>,
}

fn index(entries: Vec<Entry>) -> Index {
    let mut idx = Index {
        order: Vec::new(),
        count: HashMap::new(),
        value: HashMap::new(),
    };
    for entry in entries {
        let n = idx.count.entry(entry.key.clone()).or_insert(0);
        if *n == 0 {
            idx.order.push(entry.key.clone());
        }
        *n += 1;
        idx.value.insert(entry.key, entry.value);
    }
    idx
}

/// 基准文本里有可翻译的字母（去掉占位符之后）
fn translatable(text: &str) -> bool {
    let mut stripped = text.to_owned();
    for token in placeholders::extract(text).keys() {
        stripped = stripped.replace(token.as_str(), "");
    }
    stripped.chars().filter(|c| c.is_alphabetic()).count() >= 2
}

fn diff(base: &Placeholders, other: &Placeholders) -> (Vec<String>, Vec<String>) {
    let mut missing = Vec::new();
    let mut extra = Vec::new();
    for (token, count) in base {
        let have = other.get(token).copied().unwrap_or(0);
        missing.extend(std::iter::repeat_n(
            token.clone(),
            count.saturating_sub(have),
        ));
    }
    for (token, count) in other {
        let want = base.get(token).copied().unwrap_or(0);
        extra.extend(std::iter::repeat_n(
            token.clone(),
            count.saturating_sub(want),
        ));
    }
    (missing, extra)
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.files.len() < 2 || args.files.len() > MAX_FILES {
        return Err(PluginError::new("i18n.file_count").with("max", MAX_FILES));
    }
    let indexes = args
        .files
        .iter()
        .map(|f| f.entries().map(index))
        .collect::<PluginResult<Vec<_>>>()?;
    let base = &indexes[0];

    let mut keys: Vec<&str> = base.order.iter().map(String::as_str).collect();
    let mut seen: HashSet<&str> = keys.iter().copied().collect();
    for idx in &indexes[1..] {
        for key in &idx.order {
            if seen.insert(key.as_str()) {
                keys.push(key.as_str());
            }
        }
    }

    let mut files: Vec<Summary> = args
        .files
        .iter()
        .zip(&indexes)
        .map(|(file, idx)| Summary {
            name: file.name.clone(),
            format: file.format().ok(),
            keys: idx.order.len(),
            writable: matches!(
                file.format(),
                Ok(Format::Json | Format::Arb | Format::Properties)
            ),
            ..Summary::default()
        })
        .collect();
    let mut translated = vec![0usize; indexes.len()];

    let rows = keys
        .into_iter()
        .map(|key| {
            let base_value = base.value.get(key);
            let base_placeholders = base_value.map(|v| placeholders::extract(v));
            let cells = indexes
                .iter()
                .enumerate()
                .map(|(i, idx)| {
                    let summary = &mut files[i];
                    let mut cell = Cell {
                        value: idx.value.get(key).cloned(),
                        issues: Vec::new(),
                        missing_placeholders: Vec::new(),
                        extra_placeholders: Vec::new(),
                    };
                    if idx.count.get(key).copied().unwrap_or(0) > 1 {
                        cell.issues.push(Issue::Duplicate);
                        summary.duplicate += 1;
                    }
                    let Some(value) = &cell.value else {
                        if i > 0 && base_value.is_some() {
                            cell.issues.push(Issue::Missing);
                            summary.missing += 1;
                        }
                        return cell;
                    };
                    if i == 0 {
                        if !value.trim().is_empty() {
                            translated[i] += 1;
                        }
                        return cell;
                    }
                    let (Some(base_value), Some(base_placeholders)) =
                        (base_value, &base_placeholders)
                    else {
                        cell.issues.push(Issue::Extra);
                        summary.extra += 1;
                        return cell;
                    };
                    if value.trim().is_empty() {
                        if !base_value.trim().is_empty() {
                            cell.issues.push(Issue::Empty);
                            summary.empty += 1;
                        }
                        return cell;
                    }
                    translated[i] += 1;
                    if value == base_value && translatable(base_value) {
                        cell.issues.push(Issue::Untranslated);
                        summary.untranslated += 1;
                    }
                    let (missing, extra) = diff(base_placeholders, &placeholders::extract(value));
                    if !missing.is_empty() || !extra.is_empty() {
                        cell.issues.push(Issue::Placeholders);
                        summary.placeholders += 1;
                        cell.missing_placeholders = missing;
                        cell.extra_placeholders = extra;
                    }
                    cell
                })
                .collect();
            Row {
                key: key.to_owned(),
                cells,
            }
        })
        .collect();

    let base_keys = base.order.len();
    for (summary, done) in files.iter_mut().zip(translated) {
        summary.coverage = if base_keys == 0 {
            100.0
        } else {
            ((done.min(base_keys) as f64 / base_keys as f64) * 1000.0).round() / 10.0
        };
    }
    Ok(Output { files, rows })
}

#[cfg(test)]
#[path = "compare_test.rs"]
mod tests;
