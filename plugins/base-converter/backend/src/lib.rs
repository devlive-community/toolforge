//! 进制转换插件后端：任意精度整数在 2-36 进制间转换，并给出各位宽补码与位视图。

mod convert;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct BaseConverter {
    manifest: Manifest,
}

impl Default for BaseConverter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for BaseConverter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "convert" => to_value(convert::convert(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
