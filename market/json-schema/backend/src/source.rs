use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};
use tf_plugin_guest::net;

use crate::guard::MAX_BYTES;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Args {
    pub path: String,
}

pub fn read(args: Args) -> PluginResult<String> {
    let bytes = net::read_to_end(&args.path, MAX_BYTES as u64).map_err(|error| {
        match error.code.as_str() {
            "fs.not_a_file" => PluginError::new("schema.not_file"),
            "fs.too_large" => PluginError::new("schema.file_too_large").with("mb", 2),
            _ => PluginError::new("schema.file_read"),
        }
    })?;
    let text = String::from_utf8(bytes).map_err(|_| PluginError::new("schema.file_encoding"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
