//! MCP 工具描述与结果：由接口声明生成，不含 I/O，便于测试。

use serde_json::{Map, Value, json};
use tf_plugin_api::interface::{Effect, Interface};

use crate::i18n::Messages;

/// 工具名：`<插件>__<函数>`（MCP 工具名只允许字母、数字、`_`、`-`）
pub fn tool_name(plugin: &str, function: &str) -> String {
    format!("{plugin}__{function}")
}

pub fn parse_tool_name(name: &str) -> Option<(&str, &str)> {
    name.split_once("__")
        .filter(|(plugin, function)| !plugin.is_empty() && !function.is_empty())
}

/// 只保留标准关键字；数组元素（items）不带描述
fn clean_property(schema: &Value, messages: &Messages, item: bool) -> Value {
    let mut out = Map::new();
    for key in ["type", "enum", "default", "minimum", "maximum"] {
        if let Some(value) = schema.get(key) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    if let Some(items) = schema.get("items") {
        out.insert("items".to_owned(), clean_property(items, messages, true));
    }
    if item {
        return Value::Object(out);
    }
    let mut description = schema["description"]
        .as_str()
        .map(|d| messages.manifest_text(d))
        .unwrap_or_default();
    let is_path = schema["format"] == "path" || schema["items"]["format"] == "path";
    if is_path {
        let note = messages.text("mcp.absolutePath");
        description = if description.is_empty() {
            note
        } else {
            format!("{description} ({note})")
        };
    }
    if !description.is_empty() {
        out.insert("description".to_owned(), Value::String(description));
    }
    Value::Object(out)
}

/// 给 agent 的参数 Schema：翻译描述，去掉 `x-*` 扩展与非标准的 `format: path`
pub fn input_schema(params: &Value, messages: &Messages) -> Value {
    let properties: Map<String, Value> = params["properties"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, schema)| (name.clone(), clean_property(schema, messages, false)))
        .collect();
    let mut schema = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false
    });
    if let Some(required) = params.get("required") {
        schema["required"] = required.clone();
    }
    schema
}

fn effect_key(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "mcp.effect.read",
        Effect::Write => "mcp.effect.write",
        Effect::External => "mcp.effect.external",
    }
}

/// 一个工具的描述
pub fn tool(
    plugin: &str,
    plugin_name: &str,
    function: &str,
    interface: &Interface,
    messages: &Messages,
) -> Value {
    let summary = messages.manifest_text(&interface.summary);
    json!({
        "name": tool_name(plugin, function),
        "title": format!("{plugin_name}: {summary}"),
        "description": format!("{summary}. {}", messages.text(effect_key(interface.effect))),
        "inputSchema": input_schema(&interface.params, messages),
        "annotations": {
            "title": format!("{plugin_name}: {summary}"),
            "readOnlyHint": interface.effect == Effect::Read,
            "destructiveHint": interface.effect == Effect::Write,
            "idempotentHint": interface.effect == Effect::Read,
            "openWorldHint": interface.effect == Effect::External
        }
    })
}

/// 成功结果：结构化内容，同时以 JSON 文本提供给不读取结构化内容的客户端
pub fn success(value: &Value) -> Value {
    let mut result = json!({
        "content": [{ "type": "text", "text": value.to_string() }],
        "isError": false
    });
    if value.is_object() {
        result["structuredContent"] = value.clone();
    }
    result
}

/// 工具执行失败：作为工具结果返回，agent 能看到原因并调整
pub fn failure(message: &str, code: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": format!("{message} [{code}]") }],
        "isError": true
    })
}

#[cfg(test)]
#[path = "mcp_tools_test.rs"]
mod tests;
