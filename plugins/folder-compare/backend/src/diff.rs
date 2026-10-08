//! 两个文本文件的逐行对比，输出左右并排的行。

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use similar::{Algorithm, ChangeTag, TextDiff};
use tf_plugin_api::{PluginError, PluginResult};

/// 每侧最多读取的字节数
const MAX_SIZE: u64 = 4 * 1024 * 1024;
const MAX_ROWS: usize = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tag {
    Equal,
    Delete,
    Insert,
    Change,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Line {
    pub number: usize,
    pub text: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Row {
    pub tag: Tag,
    pub left: Option<Line>,
    pub right: Option<Line>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub rows: Vec<Row>,
    pub added: usize,
    pub removed: usize,
    pub truncated: bool,
}

/// 读取文本；没有该文件时为空，二进制或过大时报错
fn read(path: &Path) -> PluginResult<String> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Ok(String::new());
    };
    if meta.is_dir() {
        return Err(PluginError::new("fc.not_a_file"));
    }
    if meta.len() > MAX_SIZE {
        return Err(PluginError::new("fc.too_large").with("size", meta.len()));
    }
    let bytes = std::fs::read(path).map_err(|e| {
        PluginError::new("fc.read_failed")
            .with("path", path.to_string_lossy().as_ref())
            .with("detail", e.to_string())
    })?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err(PluginError::new("fc.binary"));
    }
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn line(number: usize, text: &str) -> Line {
    Line {
        number,
        text: text.trim_end_matches(['\n', '\r']).to_owned(),
    }
}

pub fn diff(left: &Path, right: &Path) -> PluginResult<Output> {
    let (a, b) = (read(left)?, read(right)?);
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .timeout(Duration::from_secs(5))
        .diff_lines(&a, &b);
    let mut rows = Vec::new();
    let (mut added, mut removed) = (0, 0);
    for op in diff.ops() {
        let changes: Vec<_> = diff.iter_changes(op).collect();
        let deletes: Vec<_> = changes
            .iter()
            .filter(|c| c.tag() == ChangeTag::Delete)
            .collect();
        let inserts: Vec<_> = changes
            .iter()
            .filter(|c| c.tag() == ChangeTag::Insert)
            .collect();
        removed += deletes.len();
        added += inserts.len();
        for c in changes.iter().filter(|c| c.tag() == ChangeTag::Equal) {
            rows.push(Row {
                tag: Tag::Equal,
                left: Some(line(c.old_index().unwrap() + 1, c.value())),
                right: Some(line(c.new_index().unwrap() + 1, c.value())),
            });
        }
        // 删除与插入成对显示为修改
        for i in 0..deletes.len().max(inserts.len()) {
            let (d, n) = (deletes.get(i), inserts.get(i));
            rows.push(Row {
                tag: match (d, n) {
                    (Some(_), Some(_)) => Tag::Change,
                    (Some(_), None) => Tag::Delete,
                    _ => Tag::Insert,
                },
                left: d.map(|c| line(c.old_index().unwrap() + 1, c.value())),
                right: n.map(|c| line(c.new_index().unwrap() + 1, c.value())),
            });
        }
    }
    let truncated = rows.len() > MAX_ROWS;
    rows.truncate(MAX_ROWS);
    Ok(Output {
        rows,
        added,
        removed,
        truncated,
    })
}

#[cfg(test)]
#[path = "diff_test.rs"]
mod tests;
