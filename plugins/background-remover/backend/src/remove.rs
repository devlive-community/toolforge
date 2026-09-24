use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{
    DynamicImage, GrayImage, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage, Rgba, RgbaImage,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::model::{Model, Sessions, predict};

const MAX_FILE: u64 = 100 * 1024 * 1024;
/// 最大像素数（约 5000 万），避免超大图占满内存
const MAX_PIXELS: u64 = 50_000_000;
const PREVIEW: u32 = 480;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Png,
    Jpeg,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub paths: Vec<String>,
    #[serde(default)]
    pub model: Model,
    /// 替换背景的颜色（#rrggbb）；为空时输出透明背景
    #[serde(default)]
    pub background: Option<String>,
    /// JPEG 只在指定背景色时可用
    #[serde(default)]
    pub format: Format,
    #[serde(default)]
    pub output_dir: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub path: String,
    pub name: String,
    pub output: Option<String>,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub ms: u64,
    /// 结果预览（PNG Data URI，保留透明）
    pub preview: Option<String>,
    /// 输出是否带透明通道（未指定背景色）
    pub transparent: bool,
    pub error: Option<PluginError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub files: Vec<FileResult>,
    pub succeeded: usize,
    pub elapsed_ms: u64,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn parse_color(value: &str) -> PluginResult<Rgb<u8>> {
    let invalid = || PluginError::new("bg.invalid_color").with("value", value);
    let hex = value.trim().trim_start_matches('#');
    let hex: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.to_owned(),
        _ => return Err(invalid()),
    };
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| invalid());
    Ok(Rgb([byte(0)?, byte(2)?, byte(4)?]))
}

/// 在输出目录中生成不覆盖已有文件的路径：`名称-nobg.扩展名`
pub fn output_path(source: &Path, dir: &Path, extension: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_owned());
    let mut candidate = dir.join(format!("{stem}-nobg.{extension}"));
    let mut index = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}-nobg ({index}).{extension}"));
        index += 1;
    }
    candidate
}

pub fn decode(bytes: &[u8]) -> PluginResult<RgbImage> {
    let failed =
        |e: image::ImageError| PluginError::new("bg.decode_failed").with("detail", e.to_string());
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PluginError::new("bg.decode_failed").with("detail", e.to_string()))?;
    if reader.format().is_none() {
        return Err(PluginError::new("bg.unsupported"));
    }
    let mut decoder = reader.into_decoder().map_err(failed)?;
    let (w, h) = decoder.dimensions();
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(PluginError::new("bg.too_many_pixels")
            .with("width", w)
            .with("height", h));
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(failed)?;
    image.apply_orientation(orientation);
    Ok(image.to_rgb8())
}

/// 用蒙版作为透明度；指定背景色时直接合成到背景上
pub fn compose(image: &RgbImage, mask: &GrayImage, background: Option<Rgb<u8>>) -> DynamicImage {
    match background {
        None => {
            DynamicImage::ImageRgba8(RgbaImage::from_fn(image.width(), image.height(), |x, y| {
                let [r, g, b] = image.get_pixel(x, y).0;
                Rgba([r, g, b, mask.get_pixel(x, y).0[0]])
            }))
        }
        Some(Rgb(bg)) => {
            DynamicImage::ImageRgb8(RgbImage::from_fn(image.width(), image.height(), |x, y| {
                let a = mask.get_pixel(x, y).0[0] as u32;
                let fg = image.get_pixel(x, y).0;
                let mix = |i: usize| ((fg[i] as u32 * a + bg[i] as u32 * (255 - a)) / 255) as u8;
                Rgb([mix(0), mix(1), mix(2)])
            }))
        }
    }
}

fn encode(image: &DynamicImage, format: Format) -> PluginResult<Vec<u8>> {
    let failed =
        |e: image::ImageError| PluginError::new("bg.encode_failed").with("detail", e.to_string());
    let mut buffer = Vec::new();
    match format {
        Format::Png => image
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .map_err(failed)?,
        Format::Jpeg => JpegEncoder::new_with_quality(&mut buffer, 92)
            .encode_image(&image.to_rgb8())
            .map_err(failed)?,
    }
    Ok(buffer)
}

fn preview(image: &DynamicImage) -> Option<String> {
    let small = if image.width().max(image.height()) > PREVIEW {
        image.resize(PREVIEW, PREVIEW, FilterType::Triangle)
    } else {
        image.clone()
    };
    let mut buffer = Vec::new();
    small
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        data_encoding::BASE64.encode(&buffer)
    ))
}

struct Done {
    output: PathBuf,
    width: u32,
    height: u32,
    bytes: u64,
    preview: Option<String>,
}

fn process(
    source: &Path,
    args: &Args,
    background: Option<Rgb<u8>>,
    plan: &crate::model::SharedPlan,
    ctx: &dyn TaskContext,
) -> PluginResult<Done> {
    let path = source.to_string_lossy();
    if ctx.file_size(&path)? > MAX_FILE {
        return Err(PluginError::new("bg.file_too_large").with("limit", "100 MB"));
    }
    let mut bytes = Vec::new();
    ctx.open_file(&path)?.read_to_end(&mut bytes).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", path.as_ref())
            .with("detail", e.to_string())
    })?;
    let image = decode(&bytes)?;
    drop(bytes);
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let mask = predict(plan, &image, args.model)?;
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let result = compose(&image, &mask, background);
    let format = if background.is_some() {
        args.format
    } else {
        Format::Png
    };
    let encoded = encode(&result, format)?;

    let dir = match args.output_dir.as_deref().filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => source.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let extension = if format == Format::Jpeg { "jpg" } else { "png" };
    let output = output_path(source, &dir, extension);
    std::fs::write(&output, &encoded).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", output.to_string_lossy().as_ref())
            .with("detail", e.to_string())
    })?;
    Ok(Done {
        output,
        width: image.width(),
        height: image.height(),
        bytes: encoded.len() as u64,
        preview: preview(&result),
    })
}

pub fn run(args: Args, sessions: &Sessions, ctx: &dyn TaskContext) -> PluginResult<Report> {
    if args.paths.is_empty() {
        return Err(PluginError::new("bg.no_files"));
    }
    if let Some(dir) = args.output_dir.as_deref().filter(|d| !d.is_empty())
        && !Path::new(dir).is_dir()
    {
        return Err(PluginError::new("bg.output_dir_missing").with("path", dir));
    }
    let background = match args.background.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(value) => Some(parse_color(value)?),
    };
    let start = Instant::now();

    ctx.stage("bg.load_model");
    let model_path = ctx.resource_path(args.model.resource_id())?;
    let load_start = Instant::now();
    let (plan, fresh) = sessions.get(args.model, &model_path)?;
    ctx.log(
        LogLevel::Info,
        if fresh {
            "bg.model_loaded"
        } else {
            "bg.model_reused"
        },
        json!({ "model": args.model.resource_id(), "ms": load_start.elapsed().as_millis() as u64 }),
    );

    ctx.stage("bg.process");
    let count = args.paths.len();
    let mut files = Vec::with_capacity(count);
    let mut succeeded = 0;
    ctx.progress(0, count as u64);
    for (index, raw) in args.paths.iter().enumerate() {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let source = Path::new(raw);
        let name = file_name(source);
        ctx.log(
            LogLevel::Info,
            "bg.file_start",
            json!({ "file": name, "index": index + 1, "count": count }),
        );
        let file_start = Instant::now();
        let outcome = process(source, &args, background, &plan, ctx);
        let ms = file_start.elapsed().as_millis() as u64;
        match outcome {
            Ok(done) => {
                succeeded += 1;
                ctx.log(
                    LogLevel::Info,
                    "bg.file_done",
                    json!({
                        "file": name,
                        "output": file_name(&done.output),
                        "width": done.width,
                        "height": done.height,
                        "ms": ms,
                    }),
                );
                files.push(FileResult {
                    path: raw.clone(),
                    name,
                    output: Some(done.output.to_string_lossy().into_owned()),
                    width: done.width,
                    height: done.height,
                    bytes: done.bytes,
                    ms,
                    preview: done.preview,
                    transparent: background.is_none(),
                    error: None,
                });
            }
            Err(err) if err.code == "task.cancelled" => return Err(err),
            Err(err) => {
                ctx.log(
                    LogLevel::Warn,
                    "bg.file_failed",
                    json!({ "file": name, "code": err.code }),
                );
                files.push(FileResult {
                    path: raw.clone(),
                    name,
                    output: None,
                    width: 0,
                    height: 0,
                    bytes: 0,
                    ms,
                    preview: None,
                    transparent: false,
                    error: Some(err),
                });
            }
        }
        ctx.progress(index as u64 + 1, count as u64);
    }

    Ok(Report {
        files,
        succeeded,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
#[path = "remove_test.rs"]
mod tests;
