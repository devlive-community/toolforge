//! User-Agent 解析器（在线安装插件）：离线规则库识别浏览器、引擎、系统、设备类型与爬虫，
//! 每项结果都标出命中的规则与匹配片段；支持批量解析与访问日志中的 UA 统计。

mod batch;
mod parse;
mod rules;

use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{
    Detection, Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct UserAgent {
    manifest: Manifest,
}

impl Default for UserAgent {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
struct ParseArgs {
    ua: String,
}

impl ToolPlugin for UserAgent {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        parse::looks_like_user_agent(text).then(|| Detection::new(85, "userAgent"))
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => to_value(parse::parse(&parse_args::<ParseArgs>(args)?.ua)),
            "analyze" => to_value(batch::run(&parse_args(args)?)?),
            "export" => to_value(batch::export(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(UserAgent);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
