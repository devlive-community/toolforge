//! 剪贴板识别：合法的 JSON 对象或数组

use serde::de::IgnoredAny;
use tf_plugin_api::Detection;

pub fn detect(text: &str) -> Option<Detection> {
    let label = match text.as_bytes().first()? {
        b'{' => "object",
        b'[' => "array",
        _ => return None,
    };
    if serde_json::from_str::<IgnoredAny>(text).is_ok() {
        return Some(Detection::new(85, label));
    }
    // 看起来像 JSON 但有语法错误，格式化工具可以指出错误位置
    (label == "object" && text.ends_with('}')).then(|| Detection::new(55, "invalid"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
