//! Protobuf 契约兼容性对比：解析两份 FileDescriptorSet，按字段号/类型名/枚举数值
//! 匹配并输出 wire 二进制（旧写新读 / 新写旧读）与 ProtoJSON 两个方向的演进风险。

mod descriptor;
mod diff;
mod report;
mod wire;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct ProtobufDiff {
    manifest: Manifest,
}

impl Default for ProtobufDiff {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ProtobufDiff {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            // 非任务路径没有 TaskContext，仅支持内联输入；文件路径输入必须走任务
            "diff" => to_value(diff::run(parse_args(args)?, None)?),
            "export" => to_value(report::export(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(
        &self,
        function: &str,
        args: Value,
        ctx: &dyn tf_plugin_api::TaskContext,
    ) -> PluginResult<Value> {
        match function {
            "diff" => to_value(diff::run(parse_args(args)?, Some(ctx))?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(ProtobufDiff);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
