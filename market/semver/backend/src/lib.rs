//! SemVer 插件（在线安装插件）：按 SemVer 2.0.0 解析、比较与排序版本，并按 npm（node-semver）范围语义批量判断命中。

mod analyze;
mod range;
mod version;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Semver {
    manifest: Manifest,
}

impl Default for Semver {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Semver {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "analyze" => to_value(analyze::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Semver);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
