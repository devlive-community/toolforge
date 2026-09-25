//! HTTP 客户端插件后端：请求由 Rust 发送（任务，可取消、实时日志），
//! 响应正文在后端格式化并缓存，前端只展示预览。

mod detect;
mod http;

use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct HttpClient {
    manifest: Manifest,
    bodies: http::Bodies,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            bodies: http::Bodies::default(),
        }
    }
}

impl ToolPlugin for HttpClient {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "to_curl" => to_value(http::to_curl(parse_args(args)?)?),
            "save_body" => to_value(self.bodies.save(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "send" => to_value(http::send(parse_args(args)?, &self.bodies, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
