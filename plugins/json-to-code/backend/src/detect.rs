//! 剪贴板识别：可以生成类型定义的 JSON 对象（或对象数组）

use serde_json::Value;
use tf_plugin_api::Detection;

pub fn detect(text: &str) -> Option<Detection> {
    if !text.starts_with(['{', '[']) {
        return None;
    }
    let value: Value = serde_json::from_str(text).ok()?;
    let object = match &value {
        Value::Object(map) => !map.is_empty(),
        Value::Array(items) => items.first().is_some_and(Value::is_object),
        _ => false,
    };
    object.then(|| Detection::new(50, "model"))
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
