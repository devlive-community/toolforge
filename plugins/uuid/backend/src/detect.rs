//! 剪贴板识别：带连字符的 UUID（避免把 32 位十六进制摘要误判为 UUID）与 ULID

use tf_plugin_api::Detection;
use uuid::Uuid;

pub fn detect(text: &str) -> Option<Detection> {
    if text.contains('-')
        && let Ok(uuid) = Uuid::try_parse(text)
    {
        return Some(Detection::new(95, "uuid").with("version", uuid.get_version_num()));
    }
    (text.len() == 26 && ulid::Ulid::from_string(text).is_ok()).then(|| Detection::new(75, "ulid"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
