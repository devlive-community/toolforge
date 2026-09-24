use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::codec::encode_base64;

const MAX_FILE: u64 = 20 * 1024 * 1024;
const CHUNK: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    #[default]
    Base64,
    DataUri,
}

#[derive(Deserialize)]
pub struct Args {
    path: String,
    #[serde(default)]
    format: Format,
    #[serde(default)]
    wrap: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    pub name: String,
    pub mime: String,
    pub bytes: u64,
}

/// 按扩展名推断 MIME 类型（常见类型即可，未知时为 application/octet-stream）
pub fn mime_for(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "txt" | "log" => "text/plain",
        "css" => "text/css",
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

pub fn encode(args: Args, ctx: &dyn TaskContext) -> PluginResult<Output> {
    let name = Path::new(&args.path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| args.path.clone());
    let size = ctx.file_size(&args.path)?;
    if size > MAX_FILE {
        return Err(PluginError::new("encode.file_too_large").with("limit", "20 MB"));
    }
    ctx.log(
        LogLevel::Info,
        "encode.file_start",
        json!({ "file": name, "size": size }),
    );

    let mut reader = ctx.open_file(&args.path)?;
    let mut bytes = Vec::with_capacity(size as usize);
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let read = reader.read(&mut buffer).map_err(|e| {
            PluginError::new("fs.io")
                .with("path", args.path.as_str())
                .with("detail", e.to_string())
        })?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        ctx.progress(bytes.len() as u64, size.max(bytes.len() as u64));
    }

    let mime = mime_for(&args.path);
    let base64 = encode_base64(&bytes, args.wrap && args.format == Format::Base64);
    let output = match args.format {
        Format::Base64 => base64,
        Format::DataUri => format!("data:{mime};base64,{base64}"),
    };
    ctx.log(
        LogLevel::Info,
        "encode.file_done",
        json!({ "file": name, "chars": output.len() }),
    );
    Ok(Output {
        output,
        name,
        mime: mime.to_owned(),
        bytes: bytes.len() as u64,
    })
}

#[cfg(test)]
#[path = "file_test.rs"]
mod tests;
