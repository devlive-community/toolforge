//! Offline, version-pinned .gitignore templates with order-preserving merging.

mod catalog;
mod generate;
mod library;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Gitignore {
    manifest: Manifest,
}

impl Default for Gitignore {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Gitignore {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "catalog" => to_value(catalog::run(parse_args(args)?)?),
            "generate" => to_value(generate::run(parse_args(args)?)?),
            "library" => to_value(library::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Gitignore);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
