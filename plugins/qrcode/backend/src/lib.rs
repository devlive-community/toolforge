//! 二维码插件后端：生成 PNG / SVG 二维码，识别图片中的二维码。

mod qr;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct QrCodeTool {
    manifest: Manifest,
}

impl Default for QrCodeTool {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for QrCodeTool {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(qr::generate(parse_args(args)?)?),
            "save" => to_value(qr::save(parse_args(args)?)?),
            "decode" => to_value(qr::decode(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
