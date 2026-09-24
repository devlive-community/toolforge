//! 编解码插件后端：Base64 / Base32 / Hex / URL / HTML 实体 / Unicode 转义，以及文件转 Base64。

mod codec;
mod file;
mod unicode;

use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Encoder {
    manifest: Manifest,
    results: file::Results,
}

impl Default for Encoder {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            results: file::Results::default(),
        }
    }
}

impl ToolPlugin for Encoder {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "transform" => to_value(codec::transform(parse_args(args)?)?),
            "file_output" => to_value(self.results.full(parse_args(args)?)?),
            "save_output" => to_value(self.results.save(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "encode_file" => {
                let output = file::encode(parse_args(args)?, ctx)?;
                to_value(self.results.store(output))
            }
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
