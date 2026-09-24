//! 去除图片背景插件后端：本地运行 U²-Net / IS-Net 分割模型（纯 Rust 推理），
//! 模型作为资源按需下载，批量处理以任务运行并输出实时日志。

mod model;
mod remove;

use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct BackgroundRemover {
    manifest: Manifest,
    sessions: model::Sessions,
}

impl Default for BackgroundRemover {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            sessions: model::Sessions::default(),
        }
    }
}

impl ToolPlugin for BackgroundRemover {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, _args: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "remove" => to_value(remove::run(parse_args(args)?, &self.sessions, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
