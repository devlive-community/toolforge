//! 剪贴板识别：能解析的 XML 文档

use serde_json::json;
use tf_plugin_api::Detection;

use crate::xml::{Mode, process};

pub fn detect(text: &str) -> Option<Detection> {
    if !text.starts_with('<') || !text.ends_with('>') {
        return None;
    }
    let args = serde_json::from_value(json!({ "input": text })).ok()?;
    process(args, Mode::Minify)
        .ok()
        .map(|_| Detection::new(85, "document"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
