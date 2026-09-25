//! IP 计算器插件后端：IPv4 / IPv6 网段计算、子网拆分、归属判断与地址段转 CIDR。

mod detect;
mod ip;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct IpCalculator {
    manifest: Manifest,
}

impl Default for IpCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for IpCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "calculate" => to_value(ip::calculate(parse_args(args)?)?),
            "range_to_cidrs" => to_value(ip::range_to_cidrs(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
