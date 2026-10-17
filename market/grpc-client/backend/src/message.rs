//! ProtoJSON 与二进制消息的互转，以及按消息类型生成可编辑的请求草稿。
//!
//! 请求 JSON 直接按描述符解析：64 位整数可以写成字符串或数字（serde_json 以任意精度保留数字，
//! 不会经过 f64 损失精度），bytes 为 base64，枚举可写名称或数字；未知字段报错。
//! 响应按 ProtoJSON 输出，64 位整数输出为字符串。

use prost::Message;
use prost_reflect::{
    DeserializeOptions, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, SerializeOptions,
};
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

/// 生成草稿时展开嵌套消息的深度
const DRAFT_DEPTH: usize = 3;

/// 解析请求 JSON 为消息；错误信息含字段路径
pub fn parse(desc: &MessageDescriptor, json: &str) -> PluginResult<DynamicMessage> {
    let text = if json.trim().is_empty() { "{}" } else { json };
    let mut de = serde_json::Deserializer::from_str(text);
    let message =
        DynamicMessage::deserialize_with_options(desc.clone(), &mut de, &DeserializeOptions::new())
            .map_err(|e| {
                PluginError::new("grpc.request_invalid")
                    .with("message", desc.full_name())
                    .with("detail", e.to_string())
            })?;
    de.end().map_err(|e| {
        PluginError::new("grpc.request_invalid")
            .with("message", desc.full_name())
            .with("detail", e.to_string())
    })?;
    Ok(message)
}

pub fn encode(message: &DynamicMessage) -> Vec<u8> {
    message.encode_to_vec()
}

/// 解码响应并输出格式化的 ProtoJSON
pub fn decode_to_json(desc: &MessageDescriptor, bytes: &[u8]) -> PluginResult<String> {
    let message = DynamicMessage::decode(desc.clone(), bytes).map_err(|e| {
        PluginError::new("grpc.response_invalid")
            .with("message", desc.full_name())
            .with("detail", e.to_string())
    })?;
    to_json(&message)
}

pub fn to_json(message: &DynamicMessage) -> PluginResult<String> {
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut out,
        serde_json::ser::PrettyFormatter::with_indent(b"  "),
    );
    message
        .serialize_with_options(&mut ser, &SerializeOptions::new())
        .map_err(|e| PluginError::new("grpc.response_invalid").with("detail", e.to_string()))?;
    Ok(String::from_utf8(out).unwrap_or_default())
}

fn scalar_default(field: &FieldDescriptor, depth: usize) -> Value {
    match field.kind() {
        Kind::Double | Kind::Float => Value::from(0.0),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 | Kind::Uint32 | Kind::Fixed32 => {
            Value::from(0)
        }
        // ProtoJSON 用字符串表示 64 位整数，避免精度损失
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 | Kind::Uint64 | Kind::Fixed64 => {
            Value::from("0")
        }
        Kind::Bool => Value::Bool(false),
        Kind::String | Kind::Bytes => Value::from(""),
        Kind::Enum(e) => e
            .values()
            .next()
            .map_or(Value::from(0), |v| Value::from(v.name())),
        Kind::Message(m) => draft_value(&m, depth + 1),
    }
}

/// 标准类型在 ProtoJSON 中有特殊写法
fn well_known(desc: &MessageDescriptor) -> Option<Value> {
    Some(match desc.full_name() {
        "google.protobuf.Timestamp" => Value::from("1970-01-01T00:00:00Z"),
        "google.protobuf.Duration" => Value::from("0s"),
        "google.protobuf.FieldMask" => Value::from(""),
        "google.protobuf.Struct" => Value::Object(Map::new()),
        "google.protobuf.ListValue" => Value::Array(Vec::new()),
        "google.protobuf.Value" | "google.protobuf.Any" | "google.protobuf.Empty" => {
            Value::Object(Map::new())
        }
        "google.protobuf.StringValue" | "google.protobuf.BytesValue" => Value::from(""),
        "google.protobuf.BoolValue" => Value::Bool(false),
        "google.protobuf.Int64Value" | "google.protobuf.UInt64Value" => Value::from("0"),
        "google.protobuf.Int32Value"
        | "google.protobuf.UInt32Value"
        | "google.protobuf.DoubleValue"
        | "google.protobuf.FloatValue" => Value::from(0),
        _ => return None,
    })
}

fn draft_value(desc: &MessageDescriptor, depth: usize) -> Value {
    if let Some(value) = well_known(desc) {
        return value;
    }
    let mut map = Map::new();
    if depth >= DRAFT_DEPTH {
        return Value::Object(map);
    }
    for field in desc.fields() {
        // oneof 只放第一个分支，避免草稿本身无效
        if let Some(oneof) = field.containing_oneof()
            && !oneof.is_synthetic()
            && oneof
                .fields()
                .next()
                .is_some_and(|first| first.number() != field.number())
        {
            continue;
        }
        let value = if field.is_map() {
            Value::Object(Map::new())
        } else if field.is_list() {
            Value::Array(Vec::new())
        } else {
            scalar_default(&field, depth)
        };
        map.insert(field.json_name().to_owned(), value);
    }
    Value::Object(map)
}

/// 按消息类型生成格式化的 ProtoJSON 草稿
pub fn draft(desc: &MessageDescriptor) -> String {
    serde_json::to_string_pretty(&draft_value(desc, 0)).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
#[path = "message_test.rs"]
mod tests;
