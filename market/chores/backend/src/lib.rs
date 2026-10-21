//! 家务轮值小管家：排期、轮换与完成状态全部在 Rust 计算，
//! 数据经宿主 SQLite 持久化（成员 / 家务 / 一次记录分键存储）。

mod date;
mod model;
mod schedule;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Chores {
    manifest: Manifest,
}

impl Default for Chores {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

/// 展开窗口内的逐次轮值、状态与成员统计
fn plan_fn(args: Value) -> PluginResult<Value> {
    let plan_args: schedule::PlanArgs = parse_args(args)?;
    to_value(schedule::plan(&plan_args)?)
}

impl ToolPlugin for Chores {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "plan" => plan_fn(args),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Chores);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
