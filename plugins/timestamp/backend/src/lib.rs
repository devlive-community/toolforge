//! 时间戳转换插件后端：时间戳与日期互转、多时区展示，基于 jiff（自带时区数据库）。

mod convert;
mod parse;
mod zone;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Timestamp {
    manifest: Manifest,
}

impl Default for Timestamp {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Timestamp {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "now" => to_value(convert::now()),
            "from_timestamp" => to_value(convert::from_timestamp(parse_args(args)?)?),
            "to_timestamp" => to_value(parse::to_timestamp(parse_args(args)?)?),
            "timezones" => to_value(zone::list()),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
