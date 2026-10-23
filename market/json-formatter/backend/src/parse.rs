use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// 严格 JSON（RFC 8259）
    #[default]
    Json,
    /// 宽松模式：允许注释、尾逗号、单引号、无引号键等
    Json5,
}

/// 解析输入；错误返回 `json.syntax` / `json.eof` / `json.data` / `json.empty`，携带 1 起始的行列号。
pub fn parse(input: &str, mode: Mode) -> PluginResult<Value> {
    if input.trim().is_empty() {
        return Err(PluginError::new("json.empty"));
    }
    match mode {
        Mode::Json => serde_json::from_str(input).map_err(|e| {
            let code = match e.classify() {
                serde_json::error::Category::Eof => "json.eof",
                serde_json::error::Category::Data => "json.data",
                _ => "json.syntax",
            };
            PluginError::new(code)
                .with("line", e.line())
                .with("column", e.column())
                .with("detail", e.to_string())
        }),
        Mode::Json5 => json5::from_str(input).map_err(|e| {
            let mut err = PluginError::new("json.syntax").with("detail", e.to_string());
            if let Some(pos) = e.position() {
                err = err
                    .with("line", pos.line + 1)
                    .with("column", pos.column + 1);
            }
            err
        }),
    }
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
