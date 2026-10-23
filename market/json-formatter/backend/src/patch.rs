//! 由两份 JSON 生成 RFC 6902 JSON Patch 与 RFC 7386 JSON Merge Patch。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tf_plugin_api::PluginResult;

use crate::format::{self, Indent};
use crate::parse::{self, Mode};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// RFC 6902
    Patch,
    /// RFC 7386
    Merge,
}

#[derive(Deserialize)]
pub struct Args {
    left: String,
    right: String,
    #[serde(default)]
    mode: Mode,
    kind: Kind,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    /// JSON Patch 的操作数；Merge Patch 为 None
    pub operations: Option<usize>,
    /// Merge Patch 无法表达的情况：右侧把值设为 null（null 表示删除）
    pub null_paths: Vec<String>,
}

/// JSON Pointer 片段转义（RFC 6901）：~ → ~0，/ → ~1
fn token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn op(name: &str, path: &str, value: Option<&Value>) -> Value {
    let mut op = json!({"op": name, "path": path});
    if let Some(value) = value {
        op["value"] = value.clone();
    }
    op
}

pub fn json_patch(left: &Value, right: &Value, path: &str, ops: &mut Vec<Value>) {
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let removed: Vec<(&String, &Value)> =
                a.iter().filter(|(k, _)| !b.contains_key(*k)).collect();
            let mut added: Vec<(&String, &Value)> =
                b.iter().filter(|(k, _)| !a.contains_key(*k)).collect();
            for (key, value) in removed {
                let from = format!("{path}/{}", token(key));
                // 同一层中被删除的值原样出现在新键上，视为改名
                if let Some(index) = added.iter().position(|(_, v)| *v == value) {
                    let (to, _) = added.remove(index);
                    ops.push(json!({"op": "move", "from": from, "path": format!("{path}/{}", token(to))}));
                } else {
                    ops.push(op("remove", &from, None));
                }
            }
            for (key, lv) in a {
                if let Some(rv) = b.get(key) {
                    json_patch(lv, rv, &format!("{path}/{}", token(key)), ops);
                }
            }
            for (key, value) in added {
                ops.push(op("add", &format!("{path}/{}", token(key)), Some(value)));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            let common = a.len().min(b.len());
            for index in 0..common {
                json_patch(&a[index], &b[index], &format!("{path}/{index}"), ops);
            }
            // 从尾部删除，避免下标移动
            for index in (common..a.len()).rev() {
                ops.push(op("remove", &format!("{path}/{index}"), None));
            }
            for (index, value) in b.iter().enumerate().skip(common) {
                ops.push(op("add", &format!("{path}/{index}"), Some(value)));
            }
        }
        (a, b) if a != b => ops.push(op("replace", path, Some(b))),
        _ => {}
    }
}

fn find_nulls(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Null => out.push(if path.is_empty() {
            "/".into()
        } else {
            path.to_owned()
        }),
        Value::Object(map) => {
            for (key, item) in map {
                find_nulls(item, &format!("{path}/{}", token(key)), out);
            }
        }
        // 数组整体替换，其中的 null 可以正常表达
        _ => {}
    }
}

/// Merge Patch：对象逐键合并，其余类型（包括数组）整体替换
pub fn merge_patch(
    left: &Value,
    right: &Value,
    path: &str,
    nulls: &mut Vec<String>,
) -> Option<Value> {
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let mut patch = Map::new();
            for key in a.keys().filter(|k| !b.contains_key(*k)) {
                patch.insert(key.clone(), Value::Null);
            }
            for (key, rv) in b {
                let child = format!("{path}/{}", token(key));
                match a.get(key) {
                    Some(lv) => {
                        if let Some(diff) = merge_patch(lv, rv, &child, nulls) {
                            patch.insert(key.clone(), diff);
                        }
                    }
                    None => {
                        find_nulls(rv, &child, nulls);
                        patch.insert(key.clone(), rv.clone());
                    }
                }
            }
            (!patch.is_empty()).then_some(Value::Object(patch))
        }
        (a, b) if a == b => None,
        (_, b) => {
            find_nulls(b, path, nulls);
            Some(b.clone())
        }
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    let left = parse::parse(&args.left, args.mode).map_err(|e| e.with("side", "left"))?;
    let right = parse::parse(&args.right, args.mode).map_err(|e| e.with("side", "right"))?;
    match args.kind {
        Kind::Patch => {
            let mut ops = Vec::new();
            json_patch(&left, &right, "", &mut ops);
            let count = ops.len();
            Ok(Output {
                output: format::pretty(&Value::Array(ops), Indent::Two)?,
                operations: Some(count),
                null_paths: Vec::new(),
            })
        }
        Kind::Merge => {
            let mut nulls = Vec::new();
            let patch =
                merge_patch(&left, &right, "", &mut nulls).unwrap_or(Value::Object(Map::new()));
            Ok(Output {
                output: format::pretty(&patch, Indent::Two)?,
                operations: None,
                null_paths: nulls,
            })
        }
    }
}

#[cfg(test)]
#[path = "patch_test.rs"]
mod tests;
