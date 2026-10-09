//! 读取并校验输入：文本或文件，可以是单个 JSON 值或每行一个值（JSON Lines）。

use serde::Deserialize;
use serde::de::IgnoredAny;
use tf_plugin_api::{PluginError, PluginResult};

/// 文件大小上限
const MAX_FILE: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Source {
    Path { path: String },
    Text { text: String },
}

pub fn load(source: &Source) -> PluginResult<String> {
    match source {
        Source::Text { text } => Ok(text.clone()),
        Source::Path { path } => {
            let meta = std::fs::metadata(path)
                .map_err(|_| PluginError::new("fs.not_found").with("path", path.as_str()))?;
            if meta.len() > MAX_FILE {
                return Err(PluginError::new("query.file_too_large").with("size", meta.len()));
            }
            let bytes = std::fs::read(path).map_err(|e| {
                PluginError::new("query.read_failed")
                    .with("path", path.as_str())
                    .with("detail", e.to_string())
            })?;
            String::from_utf8(bytes).map_err(|_| PluginError::new("query.not_utf8"))
        }
    }
}

/// 用 serde_json 校验，出错时给出行列；返回值的个数
pub fn validate(text: &str) -> PluginResult<usize> {
    if text.trim().is_empty() {
        return Err(PluginError::new("query.empty_input"));
    }
    let mut count = 0;
    for value in serde_json::Deserializer::from_str(text).into_iter::<IgnoredAny>() {
        if let Err(e) = value {
            return Err(PluginError::new("query.invalid_json")
                .with("line", e.line())
                .with("column", e.column())
                .with("detail", e.to_string()));
        }
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
