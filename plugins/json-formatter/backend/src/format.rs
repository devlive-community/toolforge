use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::ser::{PrettyFormatter, Serializer};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq)]
pub enum Indent {
    #[default]
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "4")]
    Four,
    #[serde(rename = "tab")]
    Tab,
}

impl Indent {
    fn bytes(self) -> &'static [u8] {
        match self {
            Indent::Two => b"  ",
            Indent::Four => b"    ",
            Indent::Tab => b"\t",
        }
    }
}

fn serialize_failed(e: impl ToString) -> PluginError {
    PluginError::new("json.serialize_failed").with("detail", e.to_string())
}

pub fn pretty(value: &Value, indent: Indent) -> PluginResult<String> {
    let mut out = Vec::new();
    let mut ser =
        Serializer::with_formatter(&mut out, PrettyFormatter::with_indent(indent.bytes()));
    value.serialize(&mut ser).map_err(serialize_failed)?;
    String::from_utf8(out).map_err(serialize_failed)
}

pub fn minify(value: &Value) -> PluginResult<String> {
    serde_json::to_string(value).map_err(serialize_failed)
}

/// 递归按键名排序
pub fn sort_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.sort_keys();
            map.values_mut().for_each(sort_keys);
        }
        Value::Array(items) => items.iter_mut().for_each(sort_keys),
        _ => {}
    }
}

/// 把任意文本转义为 JSON 字符串内容（不含两侧引号）
pub fn escape(input: &str) -> PluginResult<String> {
    let quoted = serde_json::to_string(input).map_err(serialize_failed)?;
    Ok(quoted[1..quoted.len() - 1].to_owned())
}

/// 反转义：输入可以带或不带两侧引号
pub fn unescape(input: &str) -> PluginResult<String> {
    let trimmed = input.trim();
    let quoted = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed.to_owned()
    } else {
        format!("\"{trimmed}\"")
    };
    serde_json::from_str::<String>(&quoted).map_err(|e| {
        PluginError::new("json.unescape_failed")
            .with("column", e.column())
            .with("detail", e.to_string())
    })
}

#[cfg(test)]
#[path = "format_test.rs"]
mod tests;
