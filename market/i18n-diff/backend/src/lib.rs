//! i18n 键值对比（在线安装插件）：对比 JSON / ARB / YAML / properties 翻译文件的缺失、多余、空值、
//! 疑似未翻译、占位符不一致与重复键，并按原格式补齐与写回。

mod compare;
mod placeholders;
mod report;
mod source;
mod write;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct I18nDiff {
    manifest: Manifest,
}

impl Default for I18nDiff {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for I18nDiff {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "compare" => to_value(compare::run(parse_args(args)?)?),
            "write" => to_value(write::run(parse_args(args)?)?),
            "report" => to_value(report::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(I18nDiff);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
