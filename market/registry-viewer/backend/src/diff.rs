//! 两份导出的差异：按键路径（不区分大小写）与值名（不区分大小写）比较。

use serde::Serialize;
use tf_plugin_api::{PluginResult, TaskContext, cancelled};

use crate::doc::{KeyNode, RegDoc, ValueEntry};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DiffKind {
    /// 仅另一文件未包含此键/值；不能据此断言线上注册表已删除
    KeyAdded,
    KeyMissing,
    ValueAdded,
    ValueMissing,
    /// 类型变化（含同字节不同类型）
    TypeChanged,
    /// 数据变化
    DataChanged,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub kind: DiffKind,
    /// 键路径（保留双方原拼写中先出现的一方）
    pub path: String,
    /// 值名；默认值为空串
    pub value_name: Option<String>,
    /// A / B 侧的类型与解读摘要
    pub a: Option<String>,
    pub b: Option<String>,
    /// A / B 侧行号
    pub line_a: Option<u32>,
    pub line_b: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffReport {
    pub entries: Vec<DiffEntry>,
    pub keys_a: usize,
    pub keys_b: usize,
    pub identical: bool,
    /// 删除指令（[-键] / 值=-）存在时为 true：文件不是最终状态快照
    pub has_delete_statements: bool,
}

fn casefold(text: &str) -> String {
    text.to_ascii_lowercase()
}

fn has_deletes(doc: &RegDoc) -> bool {
    doc.keys.iter().any(|key| {
        key.delete
            || key
                .values
                .iter()
                .any(|v| v.issues.iter().any(|issue| issue == "reg.delete_value"))
    })
}

/// 比较两份文档；键数多时按进度取消
pub fn diff(
    doc_a: &RegDoc,
    doc_b: &RegDoc,
    ctx: Option<&dyn TaskContext>,
) -> PluginResult<DiffReport> {
    let mut entries = Vec::new();

    // 键索引：casefold(path) → (原路径, KeyNode)
    let mut index_b: std::collections::HashMap<String, &KeyNode> = std::collections::HashMap::new();
    for key in &doc_b.keys {
        index_b.insert(casefold(&key.path), key);
    }
    let mut seen_b: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (index_a, key_a) in doc_a.keys.iter().enumerate() {
        if let Some(ctx) = ctx {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            if index_a % 256 == 0 {
                ctx.progress(index_a as u64, doc_a.keys.len() as u64);
            }
        }
        let folded = casefold(&key_a.path);
        let key_b = index_b.get(&folded);
        match key_b {
            None => entries.push(DiffEntry {
                kind: DiffKind::KeyMissing,
                path: key_a.path.clone(),
                value_name: None,
                a: Some(format!("values: {}", key_a.values.len())),
                b: None,
                line_a: Some(key_a.line),
                line_b: None,
            }),
            Some(key_b) => {
                seen_b.insert(folded);
                diff_values(key_a, key_b, &mut entries);
            }
        }
    }
    // B 独有键
    for key_b in &doc_b.keys {
        if !seen_b.contains(&casefold(&key_b.path)) {
            entries.push(DiffEntry {
                kind: DiffKind::KeyAdded,
                path: key_b.path.clone(),
                value_name: None,
                a: None,
                b: Some(format!("values: {}", key_b.values.len())),
                line_a: None,
                line_b: Some(key_b.line),
            });
        }
    }

    Ok(DiffReport {
        identical: entries.is_empty(),
        keys_a: doc_a.keys.len(),
        keys_b: doc_b.keys.len(),
        has_delete_statements: has_deletes(doc_a) || has_deletes(doc_b),
        entries,
    })
}

fn diff_values(key_a: &KeyNode, key_b: &KeyNode, entries: &mut Vec<DiffEntry>) {
    // 重复赋值按 Windows 语义取最后一条参与对比（后写覆盖）
    let last_a = last_by_name(&key_a.values);
    let last_b = last_by_name(&key_b.values);
    for (folded, value_a) in &last_a {
        match last_b.get(folded) {
            None => entries.push(DiffEntry {
                kind: DiffKind::ValueMissing,
                path: key_a.path.clone(),
                value_name: Some(value_a.name.clone()),
                a: Some(display_of(value_a)),
                b: None,
                line_a: Some(value_a.line),
                line_b: None,
            }),
            Some(value_b) => {
                if value_a.value.same(&value_b.value) {
                    continue;
                }
                // 同字节不同类型 / 类型一致数据不同
                let same_shape = std::mem::discriminant(&value_a.value)
                    == std::mem::discriminant(&value_b.value)
                    && type_ids_equal(&value_a.value, &value_b.value);
                entries.push(DiffEntry {
                    kind: if same_shape {
                        DiffKind::DataChanged
                    } else {
                        DiffKind::TypeChanged
                    },
                    path: key_a.path.clone(),
                    value_name: Some(value_a.name.clone()),
                    a: Some(display_of(value_a)),
                    b: Some(display_of(value_b)),
                    line_a: Some(value_a.line),
                    line_b: Some(value_b.line),
                });
            }
        }
    }
    for (folded, value_b) in &last_b {
        if !last_a.contains_key(folded) {
            entries.push(DiffEntry {
                kind: DiffKind::ValueAdded,
                path: key_a.path.clone(),
                value_name: Some(value_b.name.clone()),
                a: None,
                b: Some(display_of(value_b)),
                line_a: None,
                line_b: Some(value_b.line),
            });
        }
    }
}

/// 值名（casefold）→ 最后一条赋值
fn last_by_name(values: &[ValueEntry]) -> std::collections::HashMap<String, &ValueEntry> {
    let mut map = std::collections::HashMap::new();
    for value in values {
        map.insert(casefold(&value.name), value);
    }
    map
}

fn type_ids_equal(a: &crate::value::RegValue, b: &crate::value::RegValue) -> bool {
    let id = |value: &crate::value::RegValue| match value {
        crate::value::RegValue::Sz { .. } => 1u32,
        crate::value::RegValue::ExpandSz { .. } => 2,
        crate::value::RegValue::MultiSz { .. } => 7,
        crate::value::RegValue::Dword { .. } => 4,
        crate::value::RegValue::Qword { .. } => 11,
        crate::value::RegValue::Binary { .. } => 3,
        crate::value::RegValue::Unknown { type_id, .. } => *type_id,
    };
    id(a) == id(b)
}

fn display_of(value: &ValueEntry) -> String {
    value.value.display()
}

#[cfg(test)]
#[path = "diff_test.rs"]
mod tests;
