//! 图片拼接插件后端：把多张位图按行、列或网格拼成一张图。

mod stitch;

use std::io::Cursor;
use std::path::Path;

use data_encoding::BASE64;
use image::{ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 列表缩略图的最长边
const THUMB: u32 = 96;
/// 文件大小上限
const MAX_FILE: u64 = 200 * 1024 * 1024;

pub struct ImageStitcher {
    manifest: Manifest,
}

impl Default for ImageStitcher {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    paths: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectEntry {
    path: String,
    name: String,
    width: u32,
    height: u32,
    /// 列表缩略图（PNG Data URI）；解析失败时为空
    thumbnail: Option<String>,
    error: Option<PluginError>,
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

/// 生成列表缩略图（最长边 THUMB）
fn thumbnail(image: &RgbaImage) -> Option<String> {
    let thumb = image::imageops::thumbnail(image, THUMB, THUMB);
    let mut png = Cursor::new(Vec::new());
    thumb.write_to(&mut png, ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        BASE64.encode(png.get_ref())
    ))
}

/// 读取图片的基本信息与缩略图；单个文件失败不影响其他文件
fn inspect_one(path: &str) -> InspectEntry {
    let base = InspectEntry {
        path: path.to_owned(),
        name: file_name(path),
        width: 0,
        height: 0,
        thumbnail: None,
        error: None,
    };
    let bytes = match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE => {
            return InspectEntry {
                error: Some(PluginError::new("stitch.file_too_large").with("limit", "200 MB")),
                ..base
            };
        }
        Ok(_) => match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                return InspectEntry {
                    error: Some(
                        PluginError::new("stitch.io")
                            .with("path", path)
                            .with("detail", e.to_string()),
                    ),
                    ..base
                };
            }
        },
        Err(e) => {
            return InspectEntry {
                error: Some(
                    PluginError::new("stitch.io")
                        .with("path", path)
                        .with("detail", e.to_string()),
                ),
                ..base
            };
        }
    };
    match stitch::decode(&bytes) {
        Ok(image) => {
            let (width, height) = image.dimensions();
            InspectEntry {
                width,
                height,
                thumbnail: thumbnail(&image),
                ..base
            }
        }
        Err(err) => InspectEntry {
            error: Some(err),
            ..base
        },
    }
}

impl ToolPlugin for ImageStitcher {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => {
                let args: InspectArgs = parse_args(args)?;
                to_value(
                    args.paths
                        .iter()
                        .map(|path| inspect_one(path))
                        .collect::<Vec<_>>(),
                )
            }
            _ => Err(unknown_function(function)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "stitch" => to_value(stitch::run(parse_args(args)?, ctx)?),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
