//! 单位换算插件后端：单位表与换算都在 Rust 中，前端只负责展示。

mod units;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct UnitConverter {
    manifest: Manifest,
}

impl Default for UnitConverter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for UnitConverter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "catalog" => to_value(units::catalog()),
            "convert" => to_value(units::convert(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
