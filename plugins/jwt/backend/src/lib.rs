//! JWT 插件后端：解码（无需密钥）、签名校验与签发。
//! 令牌与密钥属于敏感数据，manifest 标记 sensitive，不写入历史。

mod claims;
mod keys;
mod token;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Jwt {
    manifest: Manifest,
}

impl Default for Jwt {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Jwt {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "decode" => to_value(token::decode(parse_args(args)?)?),
            "verify" => to_value(token::verify(parse_args(args)?)?),
            "sign" => to_value(token::sign(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
