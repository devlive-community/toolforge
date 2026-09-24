//! Markdown 编辑器插件后端：GFM 渲染、安全过滤、目录与统计，以及导出独立 HTML。

mod export;
mod render;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct MarkdownEditor {
    manifest: Manifest,
}

impl Default for MarkdownEditor {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for MarkdownEditor {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "render" => to_value(render::render(parse_args(args)?)?),
            "export_html" => to_value(export::export_html(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
