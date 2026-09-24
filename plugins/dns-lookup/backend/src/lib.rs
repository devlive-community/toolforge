//! DNS 查询插件后端：使用系统解析器或公共 DNS 查询各类记录，可同时比较多个服务器。

mod dns;

use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct DnsLookup {
    manifest: Manifest,
}

impl Default for DnsLookup {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for DnsLookup {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "lookup" => to_value(dns::lookup(parse_args(args)?)?),
            "servers" => Ok(json!({
                "types": dns::TYPES,
                "public": dns::PUBLIC
                    .iter()
                    .map(|(id, ip)| json!({ "id": id, "address": ip }))
                    .collect::<Vec<_>>(),
            })),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
