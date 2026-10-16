//! 统一社会信用代码校验器（在线安装插件）：按 GB 32100-2015 及第 1 号修改单校验长度、字符与校验位，
//! 拆分各字段并解释；只判断是否符合编码规则，不代表机构真实存在或信用状态。

mod batch;
mod code;

use serde_json::Value;
use tf_plugin_api::{
    Detection, Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Uscc {
    manifest: Manifest,
}

impl Default for Uscc {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Uscc {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<Detection> {
        let value = code::normalize(text).value;
        (value.len() == 18 && code::check(&value).status != code::Status::Invalid)
            .then(|| Detection::new(90, "uscc"))
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "validate" => to_value(batch::run(parse_args(args)?)?),
            "export" => to_value(batch::export(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Uscc);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
