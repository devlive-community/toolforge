//! 剪贴板识别：#hex 与 CSS 颜色函数

use tf_plugin_api::Detection;

use crate::color::{opaque_hex, parse};

const FUNCTIONS: &[&str] = &[
    "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
];

pub fn detect(text: &str) -> Option<Detection> {
    let lower = text.to_ascii_lowercase();
    let hex = lower.starts_with('#') && matches!(lower.len(), 4 | 5 | 7 | 9);
    let function = FUNCTIONS.iter().any(|f| lower.starts_with(f)) && lower.ends_with(')');
    if text.len() > 64 || !(hex || function) {
        return None;
    }
    let color = parse(text).ok()?;
    Some(Detection::new(92, "color").with("hex", opaque_hex(&color)))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
