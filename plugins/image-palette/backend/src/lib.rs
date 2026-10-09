//! 图片取色插件后端：读取图片（保留最近一张的解码结果），在 OKLab 空间提取主色，
//! 点击取色（可取周围平均色，附放大镜像素），并把调色板导出为 CSS / SCSS / Tailwind / JSON / GIMP 等格式。

mod color;
mod export;
mod palette;
mod pick;

use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use data_encoding::BASE64;
use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 预览图的最长边
const PREVIEW: u32 = 1600;
/// 文件大小上限
const MAX_FILE: u64 = 200 * 1024 * 1024;

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct PaletteArgs {
    path: String,
    #[serde(default = "default_count")]
    count: usize,
}

fn default_count() -> usize {
    8
}

#[derive(Deserialize)]
struct PickArgs {
    path: String,
    x: u32,
    y: u32,
    #[serde(default)]
    radius: u32,
}

#[derive(Deserialize)]
struct ExportArgs {
    colors: Vec<String>,
    format: export::Format,
    #[serde(default)]
    name: Option<String>,
}

type Key = (String, Option<SystemTime>);

pub struct ImagePalette {
    manifest: Manifest,
    /// 最近打开的图片（取色时反复使用）
    last: Mutex<Option<(Key, Arc<RgbaImage>)>>,
}

impl Default for ImagePalette {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            last: Mutex::new(None),
        }
    }
}

fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// CSS 变量等用的名称：只保留字母、数字与连字符
fn slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "palette".to_owned()
    } else {
        slug
    }
}

impl ImagePalette {
    fn image(&self, path: &str) -> PluginResult<Arc<RgbaImage>> {
        let meta = std::fs::metadata(path)
            .map_err(|_| PluginError::new("fs.not_found").with("path", path))?;
        if meta.len() > MAX_FILE {
            return Err(PluginError::new("palette.too_large").with("size", meta.len()));
        }
        let key = (path.to_owned(), meta.modified().ok());
        let mut last = self.last.lock().unwrap();
        if let Some((cached, image)) = last.as_ref()
            && *cached == key
        {
            return Ok(image.clone());
        }
        let bytes = std::fs::read(path)
            .map_err(|e| PluginError::new("palette.read_failed").with("detail", e.to_string()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|e| PluginError::new("palette.unsupported").with("detail", e.to_string()))?
            .into_rgba8();
        let image = Arc::new(image);
        *last = Some((key, image.clone()));
        Ok(image)
    }

    fn open(&self, args: PathArgs) -> PluginResult<Value> {
        let image = self.image(&args.path)?;
        let (w, h) = image.dimensions();
        let scale = (PREVIEW as f32 / w.max(h) as f32).min(1.0);
        let preview = if scale < 1.0 {
            image::imageops::resize(
                image.as_ref(),
                ((w as f32 * scale).round() as u32).max(1),
                ((h as f32 * scale).round() as u32).max(1),
                FilterType::Triangle,
            )
        } else {
            image.as_ref().clone()
        };
        let mut png = Cursor::new(Vec::new());
        preview
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|e| PluginError::new("palette.unsupported").with("detail", e.to_string()))?;
        Ok(json!({
            "path": args.path,
            "name": Path::new(&args.path).file_name().map(|n| n.to_string_lossy().into_owned()),
            "width": w,
            "height": h,
            "preview": format!("data:image/png;base64,{}", BASE64.encode(png.get_ref())),
        }))
    }
}

impl ToolPlugin for ImagePalette {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "open" => self.open(parse_args(args)?),
            "palette" => {
                let args: PaletteArgs = parse_args(args)?;
                if !(2..=24).contains(&args.count) {
                    return Err(PluginError::new("palette.invalid_count"));
                }
                let image = self.image(&args.path)?;
                to_value(palette::extract(&image, args.count))
            }
            "pick" => {
                let args: PickArgs = parse_args(args)?;
                let image = self.image(&args.path)?;
                to_value(pick::pick(&image, args.x, args.y, args.radius)?)
            }
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let colors: Vec<[u8; 3]> = args
                    .colors
                    .iter()
                    .map(|c| {
                        parse_hex(c).ok_or_else(|| {
                            PluginError::new("palette.invalid_color").with("color", c.as_str())
                        })
                    })
                    .collect::<PluginResult<_>>()?;
                let name = slug(args.name.as_deref().unwrap_or("palette"));
                Ok(json!({ "text": export::export(&colors, args.format, &name) }))
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
