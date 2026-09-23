//! UUID 插件后端：生成 UUID（v1/v3/v4/v5/v7）、ULID、NanoID，并解析已有标识。

mod generate;
mod inspect;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct UuidTool {
    manifest: Manifest,
}

impl Default for UuidTool {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for UuidTool {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate::run(parse_args(args)?)?),
            "inspect" => to_value(inspect::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
