//! 计算稿纸插件后端：逐行求值表达式，支持变量赋值、上一行结果与常见单位。

mod eval;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct ScratchCalculator {
    manifest: Manifest,
}

impl Default for ScratchCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ScratchCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "evaluate" => {
                let args: eval::Args = parse_args(args)?;
                to_value(eval::evaluate(&args.input)?)
            }
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(ScratchCalculator);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
