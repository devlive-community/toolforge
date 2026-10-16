//! 历史记录：从插件调用的参数与结果中提取要保存的输入与输出摘要。

use serde_json::Value;

/// 保存的输入上限；更长的输入截断保存
pub const MAX_INPUT: usize = 64 * 1024;
/// 输出摘要的字符数
pub const OUTPUT_PREVIEW: usize = 500;
/// 最多保留的记录数
pub const KEEP: usize = 2000;
/// 任务参数序列化后超过这个大小就不保存（无法重新运行）
pub const MAX_TASK_ARGS: usize = 64 * 1024;

/// 常见的主输入字段名，按优先级排列
const INPUT_KEYS: &[&str] = &[
    "input", "text", "source", "code", "content", "pattern", "query", "expr", "left", "url",
    "versions", "path", "value",
];

fn truncate(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    text[..text.floor_char_boundary(max_bytes)].to_owned()
}

/// 参数中的主输入：优先常见字段名，否则取最长的顶层字符串；没有非空文本时返回 None
pub fn primary_text(args: &Value) -> Option<String> {
    let map = match args {
        Value::String(s) => return (!s.trim().is_empty()).then(|| truncate(s, MAX_INPUT)),
        Value::Object(map) => map,
        _ => return None,
    };
    let named = INPUT_KEYS
        .iter()
        .filter_map(|key| map.get(*key)?.as_str())
        .find(|s| !s.trim().is_empty());
    let text = named.or_else(|| {
        map.values()
            .filter_map(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .max_by_key(|s| s.len())
    })?;
    Some(truncate(text, MAX_INPUT))
}

/// 结果的摘要：字符串结果或 output / text / result 字段的前若干字符
pub fn output_preview(result: &Value) -> Option<String> {
    let text = match result {
        Value::String(s) => s.as_str(),
        Value::Object(map) => ["output", "text", "result", "schema"]
            .iter()
            .find_map(|key| map.get(*key)?.as_str())?,
        _ => return None,
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(match trimmed.char_indices().nth(OUTPUT_PREVIEW) {
        Some((end, _)) => format!("{}…", &trimmed[..end]),
        None => trimmed.to_owned(),
    })
}

#[cfg(test)]
#[path = "history_test.rs"]
mod tests;
