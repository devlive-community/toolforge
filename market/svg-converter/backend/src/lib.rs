//! SVG 转换插件后端：把 SVG 矢量图批量栅格化为 PNG / JPEG / WebP 位图。

mod convert;
mod render;

use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct SvgConverter {
    manifest: Manifest,
}

impl Default for SvgConverter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for SvgConverter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, _args: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "convert" => to_value(convert::run(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(SvgConverter);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
