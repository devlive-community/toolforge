//! 批量把 SVG 转换为位图：解析 → 计算目标尺寸 → 栅格化 → 编码 → 写入输出目录。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder, Rgb, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};
use tf_plugin_guest::net;

use crate::render;

const MAX_FILE: u64 = 50 * 1024 * 1024;
/// 单个输出的像素预算，防止超大尺寸占满内存
const MAX_PIXELS: u64 = 40_000_000;
const THUMB: u32 = 96;
/// 超过此数量的结果不再生成缩略图，控制任务结果体积
const THUMB_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Png,
    Jpeg,
    Webp,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }
}

/// 矢量图放大不失真，所以限定宽高（fit）与百分比（scale）都允许放大
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Resize {
    #[default]
    None,
    /// 按百分比缩放（1–1000%）
    Scale { percent: u32 },
    /// 等比缩放到不超过指定宽高（0 表示该方向不限制），可以放大
    #[serde(rename_all = "camelCase")]
    Fit { max_width: u32, max_height: u32 },
}

fn default_quality() -> u8 {
    85
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub paths: Vec<String>,
    /// 输出目录；为空时输出到源文件所在目录
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub format: Format,
    /// JPEG 质量 1-100
    #[serde(default = "default_quality")]
    pub quality: u8,
    #[serde(default)]
    pub resize: Resize,
    /// 追加到输出文件名（扩展名之前）
    #[serde(default)]
    pub suffix: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub path: String,
    pub name: String,
    pub output: Option<String>,
    pub format: Option<Format>,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub new_width: u32,
    pub new_height: u32,
    /// 输出图片的小缩略图（PNG Data URI），用于结果列表预览
    pub thumbnail: Option<String>,
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

/// 计算缩放后的目标尺寸；结果至少 1 像素
pub fn target_size(width: u32, height: u32, resize: Resize) -> (u32, u32) {
    let (w, h) = match resize {
        Resize::None => (width, height),
        Resize::Scale { percent } => {
            let percent = percent.clamp(1, 1000) as u64;
            (
                (width as u64 * percent / 100) as u32,
                (height as u64 * percent / 100) as u32,
            )
        }
        Resize::Fit {
            max_width,
            max_height,
        } => {
            if max_width == 0 && max_height == 0 {
                return (width.max(1), height.max(1));
            }
            let rw = if max_width == 0 {
                f64::INFINITY
            } else {
                max_width as f64 / width as f64
            };
            let rh = if max_height == 0 {
                f64::INFINITY
            } else {
                max_height as f64 / height as f64
            };
            let ratio = rw.min(rh);
            (
                (width as f64 * ratio).round() as u32,
                (height as f64 * ratio).round() as u32,
            )
        }
    };
    (w.max(1), h.max(1))
}

/// 在输出目录中生成不与已有文件（包括源文件）冲突的路径
pub fn output_path(source: &Path, dir: &Path, suffix: &str, extension: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_owned());
    let mut candidate = dir.join(format!("{stem}{suffix}.{extension}"));
    let mut index = 1;
    while candidate == source || net::file_size(&candidate.to_string_lossy()).is_ok() {
        candidate = dir.join(format!("{stem}{suffix} ({index}).{extension}"));
        index += 1;
    }
    candidate
}

/// JPEG 不支持透明：把透明像素合成到白色背景上
fn flatten(image: &RgbaImage) -> RgbImage {
    RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        let blend = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        Rgb([blend(r), blend(g), blend(b)])
    })
}

fn encode_failed(err: impl ToString) -> PluginError {
    PluginError::new("svg.encode_failed").with("detail", err.to_string())
}

pub fn encode(image: &RgbaImage, format: Format, quality: u8) -> PluginResult<Vec<u8>> {
    let mut buffer = Vec::new();
    let (width, height) = (image.width(), image.height());
    match format {
        Format::Png => {
            PngEncoder::new_with_quality(&mut buffer, CompressionType::Best, PngFilter::Adaptive)
                .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
                .map_err(encode_failed)?
        }
        Format::Jpeg => JpegEncoder::new_with_quality(&mut buffer, quality.clamp(1, 100))
            .write_image(
                flatten(image).as_raw(),
                width,
                height,
                ExtendedColorType::Rgb8,
            )
            .map_err(encode_failed)?,
        // image 里的 WebP 编码器只有无损，alpha 可以保留
        Format::Webp => WebPEncoder::new_lossless(&mut buffer)
            .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
            .map_err(encode_failed)?,
    }
    Ok(buffer)
}

fn thumbnail(image: &RgbaImage) -> Option<String> {
    let thumb = image::imageops::thumbnail(image, THUMB, THUMB);
    let mut buffer = Vec::new();
    PngEncoder::new(&mut buffer)
        .write_image(
            thumb.as_raw(),
            thumb.width(),
            thumb.height(),
            ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        data_encoding::BASE64.encode(&buffer)
    ))
}

struct Converted {
    output: PathBuf,
    format: Format,
    after_bytes: u64,
    size: (u32, u32),
    new_size: (u32, u32),
    thumbnail: Option<String>,
}

fn convert_one(
    source: &Path,
    args: &Args,
    ctx: &dyn TaskContext,
    with_thumbnail: bool,
) -> PluginResult<Converted> {
    let path = source.to_string_lossy();
    if ctx.file_size(&path)? > MAX_FILE {
        return Err(PluginError::new("svg.file_too_large").with("limit", "50 MB"));
    }
    let mut bytes = Vec::new();
    ctx.open_file(&path)?.read_to_end(&mut bytes).map_err(|e| {
        PluginError::new("svg.io")
            .with("path", path.as_ref())
            .with("detail", e.to_string())
    })?;

    let tree = render::parse(&bytes)?;
    drop(bytes);
    let declared = tree.size();
    let size = (
        declared.width().round().max(1.0) as u32,
        declared.height().round().max(1.0) as u32,
    );
    let new_size = target_size(size.0, size.1, args.resize);
    if new_size.0 as u64 * new_size.1 as u64 > MAX_PIXELS {
        return Err(PluginError::new("svg.pixels_too_large")
            .with("width", new_size.0 as u64)
            .with("height", new_size.1 as u64));
    }

    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let format = args.format;
    let image = render::render(&tree, new_size.0, new_size.1)?;
    let encoded = encode(&image, format, args.quality)?;

    let dir = match args.output_dir.as_deref().filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => source.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let output = output_path(source, &dir, &args.suffix, format.extension());
    net::write_file(&output.to_string_lossy(), &encoded).map_err(|error| {
        let detail = error
            .params
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or(&error.code);
        PluginError::new("svg.io")
            .with("path", output.to_string_lossy().as_ref())
            .with("detail", detail)
    })?;

    Ok(Converted {
        output,
        format,
        after_bytes: encoded.len() as u64,
        size,
        new_size,
        thumbnail: with_thumbnail.then(|| thumbnail(&image)).flatten(),
    })
}

/// 批量转换：逐个解析 → 栅格化 → 编码 → 写入输出目录。
/// 单个文件失败不影响其他文件，每个文件输出实时日志，按文件数汇报进度。
pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    if args.paths.is_empty() {
        return Err(PluginError::new("svg.no_files"));
    }
    // 沙箱内无法预先检查目录，目录不存在时每个文件各自报告写入失败
    let start = Instant::now();
    let count = args.paths.len();
    let mut files = Vec::with_capacity(count);
    let mut succeeded = 0usize;

    ctx.stage("svg.convert");
    for (index, raw) in args.paths.iter().enumerate() {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let source = Path::new(raw);
        let name = file_name(source);
        let before_bytes = ctx.file_size(raw).unwrap_or(0);
        ctx.log(
            LogLevel::Info,
            "svg.file_start",
            json!({ "file": name, "index": index + 1, "count": count }),
        );
        let file_start = Instant::now();

        match convert_one(source, &args, ctx, index < THUMB_LIMIT) {
            Ok(done) => {
                succeeded += 1;
                ctx.log(
                    LogLevel::Info,
                    "svg.file_done",
                    json!({
                        "file": name,
                        "output": file_name(&done.output),
                        "width": done.new_size.0,
                        "height": done.new_size.1,
                        "after": done.after_bytes,
                        "ms": file_start.elapsed().as_millis() as u64,
                    }),
                );
                files.push(FileResult {
                    path: raw.clone(),
                    name,
                    output: Some(done.output.to_string_lossy().into_owned()),
                    format: Some(done.format),
                    before_bytes,
                    after_bytes: done.after_bytes,
                    width: done.size.0,
                    height: done.size.1,
                    new_width: done.new_size.0,
                    new_height: done.new_size.1,
                    thumbnail: done.thumbnail,
                    error: None,
                });
            }
            Err(err) if err.code == "task.cancelled" => return Err(err),
            Err(err) => {
                ctx.log(
                    LogLevel::Warn,
                    "svg.file_failed",
                    json!({ "file": name, "code": err.code }),
                );
                files.push(FileResult {
                    path: raw.clone(),
                    name,
                    output: None,
                    format: None,
                    before_bytes,
                    after_bytes: 0,
                    width: 0,
                    height: 0,
                    new_width: 0,
                    new_height: 0,
                    thumbnail: None,
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
#[path = "convert_test.rs"]
mod tests;
