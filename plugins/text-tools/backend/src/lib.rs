//! 文本处理插件后端：命名风格 / 大小写转换、按行处理与文本统计。

mod case;
mod lines;
mod stats;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TextTools {
    manifest: Manifest,
}

impl Default for TextTools {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TextTools {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "cases" => to_value(case::all(parse_args(args)?)),
            "lines" => to_value(lines::process(parse_args(args)?)),
            "stats" => to_value(stats::collect(parse_args(args)?)),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
