//! 正则测试插件后端：基于 fancy-regex（支持断言与反向引用），
//! 匹配位置换算为 UTF-16 偏移以便前端编辑器直接高亮。

mod engine;
mod offsets;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Regex {
    manifest: Manifest,
}

impl Default for Regex {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Regex {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "test" => to_value(engine::test(parse_args(args)?)?),
            "replace" => to_value(engine::replace(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
