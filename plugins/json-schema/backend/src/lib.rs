//! Offline JSON Schema validation with explicit dialects and bounded inputs,
//! plus schema inference from JSON samples.

mod guard;
mod infer;
mod source;
mod validate;
mod work;

use serde_json::Value;
use tf_plugin_api::{
    Detection, Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct JsonSchema {
    manifest: Manifest,
}

impl Default for JsonSchema {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for JsonSchema {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        if text.len() > tf_plugin_api::DETECT_LIMIT {
            return None;
        }
        let schema: Value = serde_json::from_str(text).ok()?;
        let uri = schema.as_object()?.get("$schema")?.as_str()?;
        matches!(
            uri.trim_end_matches('#'),
            "https://json-schema.org/draft/2020-12/schema"
                | "http://json-schema.org/draft-07/schema"
                | "https://json-schema.org/draft-07/schema"
        )
        .then(|| {
            Detection::new(
                96,
                if uri.contains("draft-07") {
                    "schema7"
                } else {
                    "schema"
                },
            )
        })
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "read_file" => to_value(source::read(parse_args(args)?)?),
            "infer" => to_value(infer::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "validate" => to_value(validate::run(parse_args(args)?, ctx)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
