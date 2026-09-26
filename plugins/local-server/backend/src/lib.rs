//! 本地静态文件服务插件后端：把文件夹作为网站在本机或局域网内提供访问，实时记录请求。

mod addresses;
mod handler;
mod serve;

use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct FreePortArgs {
    #[serde(default = "default_port")]
    start: u16,
    #[serde(default)]
    lan: bool,
}

fn default_port() -> u16 {
    8000
}

pub struct LocalServer {
    manifest: Manifest,
}

impl Default for LocalServer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for LocalServer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "addresses" => to_value(addresses::addresses(parse_args(args)?)?),
            "free_port" => {
                let args: FreePortArgs = parse_args(args)?;
                to_value(addresses::free_port(args.start, args.lan)?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "serve" => to_value(serve::run(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}
