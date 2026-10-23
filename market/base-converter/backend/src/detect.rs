//! 剪贴板识别：带 0x / 0b / 0o 前缀的整数

use tf_plugin_api::Detection;

use crate::convert::parse;

pub fn detect(text: &str) -> Option<Detection> {
    let body = text.strip_prefix('-').unwrap_or(text);
    let prefix = body.get(..2)?.to_ascii_lowercase();
    if !matches!(prefix.as_str(), "0x" | "0b" | "0o") || body.len() < 3 || body.len() > 130 {
        return None;
    }
    let (_, base) = parse(text, 0).ok()?;
    Some(Detection::new(80, "number").with("base", base))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
