//! XML 格式化插件后端：基于 quick-xml 的格式化、压缩与良构性校验。

mod detect;
mod xml;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct XmlFormatter {
    manifest: Manifest,
}

impl Default for XmlFormatter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for XmlFormatter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "format" => to_value(xml::process(parse_args(args)?, xml::Mode::Pretty)?),
            "minify" => to_value(xml::process(parse_args(args)?, xml::Mode::Minify)?),
            "validate" => to_value(xml::process(parse_args(args)?, xml::Mode::Pretty)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
