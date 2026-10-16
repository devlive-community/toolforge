//! 支付测试卡号生成与校验（在线安装插件）：按卡组织号段、长度与 Luhn 规则生成本地格式测试数据，
//! 校验卡号并说明失败原因，内置有来源的支付平台官方沙箱测试卡。只用于开发测试。

mod brand;
mod card;
mod sandbox;

use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TestCard {
    manifest: Manifest,
}

impl Default for TestCard {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TestCard {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(card::generate(parse_args(args)?)?),
            "validate" => to_value(card::validate(parse_args(args)?)?),
            "sandbox" => Ok(
                json!({ "cases": sandbox::CASES, "source": sandbox::STRIPE_SOURCE, "verified": sandbox::VERIFIED }),
            ),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TestCard);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
