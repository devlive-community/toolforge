use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::guard::MAX_BYTES;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Args {
    pub path: String,
}

pub fn read(args: Args) -> PluginResult<String> {
    let file = File::open(&args.path).map_err(|_| PluginError::new("schema.file_read"))?;
    let metadata = file
        .metadata()
        .map_err(|_| PluginError::new("schema.file_read"))?;
    if !metadata.is_file() {
        return Err(PluginError::new("schema.not_file"));
    }
    if metadata.len() > MAX_BYTES as u64 {
        return Err(PluginError::new("schema.file_too_large").with("mb", 2));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PluginError::new("schema.file_read"))?;
    if bytes.len() > MAX_BYTES {
        return Err(PluginError::new("schema.file_too_large").with("mb", 2));
    }
    let text = String::from_utf8(bytes).map_err(|_| PluginError::new("schema.file_encoding"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
