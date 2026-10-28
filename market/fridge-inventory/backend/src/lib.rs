//! 冰箱食材与到期提醒：批次台账。余量守恒、日期对比与数量事务都在 Rust 完成，
//! 数据经宿主 SQLite 持久化（批次 / 变动 / 设置分键存储），不新建数据库。

mod date;
mod flat;
mod model;
mod ops;
mod review;
mod units;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 后端分发的全部函数；宿主只放行 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 4] = ["review", "apply", "expiry", "convert"];

pub struct FridgeInventory {
    manifest: Manifest,
}

impl Default for FridgeInventory {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

/// 汇总每批余量、状态与日期分档
fn review_fn(args: Value) -> PluginResult<Value> {
    let plan_args: review::ReviewArgs = parse_args(args)?;
    to_value(review::review(&plan_args)?)
}

/// 数量事务：消耗 / 丢弃 / 调整入库量，校验通过才返回落账数据
fn apply_fn(args: Value) -> PluginResult<Value> {
    let apply_args: ops::ApplyArgs = parse_args(args)?;
    to_value(ops::apply(&apply_args)?)
}

impl ToolPlugin for FridgeInventory {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "review" => review_fn(args),
            "apply" => apply_fn(args),
            "expiry" => to_value(flat::expiry(parse_args(args)?)?),
            "convert" => flat::convert(parse_args(args)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(FridgeInventory);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
