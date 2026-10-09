//! JSONPath（RFC 9535）查询，结果附带规范化路径。

use serde_json::Value;
use serde_json_path::JsonPath;
use tf_plugin_api::{PluginError, PluginResult};

use crate::{Options, Output, Results};

/// 工作区开启了 preserve_order，需要逐层排序
fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k.clone(), sort_keys(v)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

fn format(value: &Value, options: &Options) -> String {
    if options.raw
        && let Value::String(s) = value
    {
        return s.clone();
    }
    let sorted;
    let value = if options.sort_keys {
        sorted = sort_keys(value);
        &sorted
    } else {
        value
    };
    if options.compact {
        serde_json::to_string(value)
    } else {
        serde_json::to_string_pretty(value)
    }
    .unwrap_or_default()
}

pub fn run(query: &str, input: &str, options: &Options) -> PluginResult<Results> {
    if query.trim().is_empty() {
        return Err(PluginError::new("query.empty_query"));
    }
    let path = JsonPath::parse(query.trim()).map_err(|e| {
        PluginError::new("query.jsonpath")
            .with("detail", e.to_string())
            .with("column", e.position() + 1)
    })?;
    let mut results = Results::default();
    for value in serde_json::Deserializer::from_str(input).into_iter::<Value>() {
        let value = value
            .map_err(|e| PluginError::new("query.invalid_json").with("detail", e.to_string()))?;
        for node in path.query_located(&value).all() {
            let output = Output {
                path: Some(node.location().to_string()),
                text: format(node.node(), options),
            };
            if !results.push(output) {
                return Ok(results);
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
#[path = "jsonpath_test.rs"]
mod tests;
