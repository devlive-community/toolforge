//! 图片压缩插件后端：mozjpeg / 调色板量化 + oxipng / libwebp 批量压缩，支持对比原图与压缩结果。

mod batch;
mod engine;

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 对比预览时单个文件的大小上限
const MAX_PREVIEW: u64 = 30 * 1024 * 1024;

#[derive(Deserialize)]
struct ScanArgs {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct CompareArgs {
    original: String,
    output: String,
}

#[derive(Serialize)]
struct Compare {
    original: String,
    output: String,
}

fn data_uri(path: &str) -> PluginResult<String> {
    let size = std::fs::metadata(path)
        .map_err(|_| PluginError::new("fs.not_found").with("path", path))?
        .len();
    if size > MAX_PREVIEW {
        return Err(PluginError::new("image.too_large").with("limit", "30 MB"));
    }
    let bytes = std::fs::read(path).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", path)
            .with("detail", e.to_string())
    })?;
    let format = engine::decode_format(&bytes)?;
    Ok(format!(
        "data:{};base64,{}",
        format.mime(),
        data_encoding::BASE64.encode(&bytes)
    ))
}

pub struct ImageCompressor {
    manifest: Manifest,
}

impl Default for ImageCompressor {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ImageCompressor {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "scan" => {
                let args: ScanArgs = parse_args(args)?;
                to_value(batch::scan(&args.paths))
            }
            "compare" => {
                let args: CompareArgs = parse_args(args)?;
                if !Path::new(&args.output).is_file() {
                    return Err(PluginError::new("fs.not_found").with("path", args.output));
                }
                to_value(Compare {
                    original: data_uri(&args.original)?,
                    output: data_uri(&args.output)?,
                })
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "compress" => to_value(batch::run(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}
