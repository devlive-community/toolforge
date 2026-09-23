use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::docs::DocCache;

const PREVIEW_CHARS: usize = 120;
const DEFAULT_LIMIT: usize = 200;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Segment {
    Index(usize),
    Key(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    doc_id: u64,
    #[serde(default)]
    path: Vec<Segment>,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
}

#[derive(Debug, Serialize)]
pub struct Node {
    pub key: Segment,
    pub kind: Kind,
    /// 容器的子元素数量
    pub size: usize,
    /// 基本类型的展示文本（字符串带引号，过长截断）
    pub preview: String,
}

#[derive(Debug, Serialize)]
pub struct Page {
    /// 目标节点自身的类型
    pub kind: Kind,
    pub nodes: Vec<Node>,
    pub total: usize,
}

fn kind_of(value: &Value) -> (Kind, usize) {
    match value {
        Value::Object(map) => (Kind::Object, map.len()),
        Value::Array(items) => (Kind::Array, items.len()),
        Value::String(_) => (Kind::String, 0),
        Value::Number(_) => (Kind::Number, 0),
        Value::Bool(_) => (Kind::Boolean, 0),
        Value::Null => (Kind::Null, 0),
    }
}

fn node(key: Segment, value: &Value) -> Node {
    let (kind, size) = kind_of(value);
    let preview = match value {
        Value::Object(_) | Value::Array(_) => String::new(),
        other => truncate(&other.to_string()),
    };
    Node {
        key,
        kind,
        size,
        preview,
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= PREVIEW_CHARS {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(PREVIEW_CHARS).collect();
    cut.push('…');
    cut
}

fn resolve<'a>(root: &'a Value, path: &[Segment]) -> Option<&'a Value> {
    path.iter()
        .try_fold(root, |value, segment| match (value, segment) {
            (Value::Object(map), Segment::Key(key)) => map.get(key),
            (Value::Array(items), Segment::Index(index)) => items.get(*index),
            _ => None,
        })
}

/// 分页返回某个路径下的直接子节点；path 为空表示根节点
pub fn children(docs: &DocCache, args: Args) -> PluginResult<Page> {
    let doc = docs
        .get(args.doc_id)
        .ok_or_else(|| PluginError::new("json.doc_expired"))?;
    let target =
        resolve(&doc, &args.path).ok_or_else(|| PluginError::new("json.path_not_found"))?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let (nodes, total) = match target {
        Value::Object(map) => (
            map.iter()
                .skip(args.offset)
                .take(limit)
                .map(|(k, v)| node(Segment::Key(k.clone()), v))
                .collect(),
            map.len(),
        ),
        Value::Array(items) => (
            items
                .iter()
                .enumerate()
                .skip(args.offset)
                .take(limit)
                .map(|(i, v)| node(Segment::Index(i), v))
                .collect(),
            items.len(),
        ),
        _ => (Vec::new(), 0),
    };
    Ok(Page {
        kind: kind_of(target).0,
        nodes,
        total,
    })
}

#[cfg(test)]
#[path = "tree_test.rs"]
mod tests;
