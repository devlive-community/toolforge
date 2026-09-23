//! SQL 格式化插件后端：基于 sqlformat 的格式化，以及保留字符串内容的压缩。

mod format;
mod scan;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct SqlFormatter {
    manifest: Manifest,
}

impl Default for SqlFormatter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for SqlFormatter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "format" => to_value(format::format(parse_args(args)?)?),
            "minify" => to_value(format::minify(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
