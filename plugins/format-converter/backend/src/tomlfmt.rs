//! TOML 与 JSON 数据模型之间的转换（日期保持为字符串，null 无法表示）。

use serde_json::{Map, Number, Value};
use tf_plugin_api::{PluginError, PluginResult};

pub fn to_json(value: toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(items) => Value::Array(items.into_iter().map(to_json).collect()),
        toml::Value::Table(table) => Value::Object(
            table
                .into_iter()
                .map(|(k, v)| (k, to_json(v)))
                .collect::<Map<_, _>>(),
        ),
    }
}

fn path_of(path: &[String]) -> String {
    if path.is_empty() {
        "$".to_owned()
    } else {
        format!("$.{}", path.join("."))
    }
}

pub fn from_json(value: &Value, path: &mut Vec<String>) -> PluginResult<toml::Value> {
    Ok(match value {
        Value::Null => {
            return Err(PluginError::new("format.toml_null").with("path", path_of(path)));
        }
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml::Value::Integer(i),
            (None, Some(f)) => toml::Value::Float(f),
            _ => return Err(PluginError::new("format.toml_number").with("path", path_of(path))),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                path.push(index.to_string());
                out.push(from_json(item, path)?);
                path.pop();
            }
            toml::Value::Array(out)
        }
        Value::Object(map) => {
            let mut table = toml::Table::new();
            for (key, item) in map {
                path.push(key.clone());
                table.insert(key.clone(), from_json(item, path)?);
                path.pop();
            }
            toml::Value::Table(table)
        }
    })
}
