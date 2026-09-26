//! 文字识别插件后端：本地运行 PP-OCRv4 检测与识别模型（纯 Rust 推理），
//! 支持中英文混排；模型作为资源按需下载，识别以任务运行并输出实时日志。

mod detect;
mod engine;
mod ocr;
mod recognize;

use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Ocr {
    manifest: Manifest,
    engines: engine::Engines,
}

impl Default for Ocr {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            engines: engine::Engines::default(),
        }
    }
}

impl ToolPlugin for Ocr {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect_image(&self, width: u32, height: u32) -> Option<tf_plugin_api::Detection> {
        (width.min(height) >= 16).then(|| tf_plugin_api::Detection::new(80, "image"))
    }

    fn call(&self, function: &str, _args: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "recognize" => to_value(ocr::recognize(parse_args(args)?, &self.engines, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
