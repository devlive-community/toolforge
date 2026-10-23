//! 图片信息插件后端：尺寸与格式、EXIF（含 GPS）、主色，以及无损移除元数据。

mod inspect;
mod palette;
mod strip;
#[cfg(test)]
mod test_support;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct ImageInfo {
    manifest: Manifest,
}

impl Default for ImageInfo {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ImageInfo {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => to_value(inspect::inspect(parse_args(args)?)?),
            "strip_metadata" => to_value(inspect::strip_metadata(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
