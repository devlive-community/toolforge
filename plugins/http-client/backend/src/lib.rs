//! HTTP 客户端插件后端：请求由 Rust 发送（任务，可取消、实时日志），
//! 响应正文在后端格式化并缓存，前端只展示预览。

mod detect;
mod http;
mod import;
mod workspace;

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
            "to_curl" => {
                let args: workspace::RequestArgs = parse_args(args)?;
                to_value(http::to_curl(args.resolve()?).map_err(|error| args.protect_error(error))?)
            }
            "save_body" => to_value(self.bodies.save(parse_args(args)?)?),
            "workspace_validate" => {
                let args: workspace::WorkspaceArgs = parse_args(args)?;
                to_value(workspace::validate(args.workspace)?)
            }
            "new_id" => to_value(workspace::new_id()),
            "resolve_request" => {
                let args: workspace::ResolveArgs = parse_args(args)?;
                to_value(workspace::resolve(args.request, &args.variables)?)
            }
            "import_curl" => {
                let args: import::ImportArgs = parse_args(args)?;
                to_value(import::import(&args.command)?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "send" => {
                let args: workspace::RequestArgs = parse_args(args)?;
                let request = args.resolve()?;
                let result = if args.variables.is_empty() {
                    http::send(request, &self.bodies, ctx)
                } else {
                    http::send_with_redacted_urls(request, &self.bodies, ctx, true)
                };
                to_value(result.map_err(|error| args.protect_error(error))?)
            }
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
