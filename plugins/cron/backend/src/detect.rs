//! 剪贴板识别：5–7 段的 cron 表达式与 @daily 等别名

use tf_plugin_api::Detection;

use crate::cron::is_valid;

const ALIASES: &[&str] = &[
    "@yearly",
    "@annually",
    "@monthly",
    "@weekly",
    "@daily",
    "@midnight",
    "@hourly",
];

pub fn detect(text: &str) -> Option<Detection> {
    if ALIASES.contains(&text.to_ascii_lowercase().as_str()) {
        return Some(Detection::new(85, "alias"));
    }
    let fields: Vec<&str> = text.split_whitespace().collect();
    let shaped = (5..=7).contains(&fields.len())
        && fields.iter().all(|f| {
            f.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"*/,-?#".contains(&b))
        })
        // 纯数字的几个词（如「1 2 3 4 5」）不当作 cron
        && fields
            .iter()
            .any(|f| f.bytes().any(|b| !b.is_ascii_digit()));
    (shaped && is_valid(text))
        .then(|| Detection::new(90, "expression").with("fields", fields.len()))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
