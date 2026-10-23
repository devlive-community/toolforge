//! 剪贴板识别：YAML 与 TOML 文档（JSON 由 JSON 格式化工具优先处理）

use tf_plugin_api::Detection;

use crate::convert::{Format, detect as guess};

pub fn detect(text: &str) -> Option<Detection> {
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    match guess(text) {
        Format::Json => Some(Detection::new(40, "json")),
        Format::Toml if lines >= 2 => Some(Detection::new(70, "toml")),
        Format::Yaml if lines >= 2 => {
            let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).ok()?;
            (value.is_mapping() || value.is_sequence()).then(|| Detection::new(60, "yaml"))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
