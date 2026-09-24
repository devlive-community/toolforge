use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

const MAX_FILE: u64 = 200 * 1024 * 1024;
const ICO_MAX: u32 = 256;
const THUMB: u32 = 96;
/// 超过此数量的结果不再生成缩略图，控制任务结果体积
const THUMB_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    Keep,
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
    Ico,
    Tiff,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Resize {
    #[default]
    None,
    /// 按百分比缩放
    Scale { percent: u32 },
    /// 等比缩放到不超过指定宽高（0 表示该方向不限制），不放大
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
    pub target: Target,
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
    pub format: Option<Target>,
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
    pub total_before: u64,
    pub total_after: u64,
    pub elapsed_ms: u64,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Target {
    fn from_format(format: ImageFormat) -> Option<Self> {
        Some(match format {
            ImageFormat::Png => Self::Png,
            ImageFormat::Jpeg => Self::Jpeg,
            ImageFormat::WebP => Self::Webp,
            ImageFormat::Gif => Self::Gif,
            ImageFormat::Bmp => Self::Bmp,
            ImageFormat::Ico => Self::Ico,
            ImageFormat::Tiff => Self::Tiff,
            _ => return None,
        })
    }

    fn format(self) -> ImageFormat {
        match self {
            Self::Keep | Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Webp => ImageFormat::WebP,
            Self::Gif => ImageFormat::Gif,
            Self::Bmp => ImageFormat::Bmp,
            Self::Ico => ImageFormat::Ico,
            Self::Tiff => ImageFormat::Tiff,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Keep | Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Ico => "ico",
            Self::Tiff => "tiff",
        }
    }
}

/// 计算缩放后的尺寸；结果至少 1 像素
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
            let rw = if max_width == 0 {
                1.0
            } else {
                max_width as f64 / width as f64
            };
            let rh = if max_height == 0 {
                1.0
            } else {
                max_height as f64 / height as f64
            };
            let ratio = rw.min(rh).min(1.0);
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
    while candidate.exists() || candidate == source {
        candidate = dir.join(format!("{stem}{suffix} ({index}).{extension}"));
        index += 1;
    }
    candidate
}

/// JPEG 不支持透明：把透明像素合成到白色背景上
fn flatten(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        Rgb([blend(r), blend(g), blend(b)])
    })
}

fn encode_failed(err: impl ToString) -> PluginError {
    PluginError::new("image.encode_failed").with("detail", err.to_string())
}

pub fn encode(image: &DynamicImage, target: Target, quality: u8) -> PluginResult<Vec<u8>> {
    let mut buffer = Vec::new();
    match target {
        Target::Jpeg => {
            JpegEncoder::new_with_quality(&mut buffer, quality.clamp(1, 100))
                .encode_image(&flatten(image))
                .map_err(encode_failed)?;
        }
        Target::Png | Target::Keep => {
            let encoder = PngEncoder::new_with_quality(
                &mut buffer,
                CompressionType::Best,
                PngFilter::Adaptive,
            );
            normalize(image)
                .write_with_encoder(encoder)
                .map_err(encode_failed)?;
        }
        _ => {
            // 其余编码器只接受 8 位 RGB(A)，GIF / ICO 需要 RGBA
            let image = match target {
                Target::Gif | Target::Ico => DynamicImage::ImageRgba8(image.to_rgba8()),
                _ => normalize(image),
            };
            image
                .write_to(&mut Cursor::new(&mut buffer), target.format())
                .map_err(encode_failed)?;
        }
    }
    Ok(buffer)
}

/// 统一为 8 位 RGB / RGBA，兼容 16 位与浮点图像
fn normalize(image: &DynamicImage) -> DynamicImage {
    if image.color().has_alpha() {
        DynamicImage::ImageRgba8(image.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
    }
}

pub fn decode(bytes: &[u8]) -> PluginResult<(DynamicImage, ImageFormat)> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PluginError::new("image.decode_failed").with("detail", e.to_string()))?;
    let format = reader
        .format()
        .ok_or_else(|| PluginError::new("image.unsupported"))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| PluginError::new("image.decode_failed").with("detail", e.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|e| PluginError::new("image.decode_failed").with("detail", e.to_string()))?;
    // 按 EXIF 方向摆正，重新编码后方向信息会丢失
    image.apply_orientation(orientation);
    Ok((image, format))
}

fn thumbnail(image: &DynamicImage) -> Option<String> {
    let thumb = image.thumbnail(THUMB, THUMB);
    let mut buffer = Vec::new();
    DynamicImage::ImageRgba8(thumb.to_rgba8())
        .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        data_encoding::BASE64.encode(&buffer)
    ))
}

struct Converted {
    output: PathBuf,
    target: Target,
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
        return Err(PluginError::new("image.too_large").with("limit", "200 MB"));
    }
    let mut bytes = Vec::new();
    ctx.open_file(&path)?.read_to_end(&mut bytes).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", path.as_ref())
            .with("detail", e.to_string())
    })?;

    let (image, format) = decode(&bytes)?;
    drop(bytes);
    let size = (image.width(), image.height());
    let target = match args.target {
        Target::Keep => Target::from_format(format).unwrap_or(Target::Png),
        other => other,
    };

    let mut new_size = target_size(size.0, size.1, args.resize);
    if target == Target::Ico && (new_size.0 > ICO_MAX || new_size.1 > ICO_MAX) {
        new_size = target_size(
            new_size.0,
            new_size.1,
            Resize::Fit {
                max_width: ICO_MAX,
                max_height: ICO_MAX,
            },
        );
        ctx.log(
            LogLevel::Warn,
            "image.ico_fitted",
            json!({ "file": file_name(source), "max": ICO_MAX }),
        );
    }
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let image = if new_size == size {
        image
    } else {
        image.resize_exact(new_size.0, new_size.1, FilterType::Lanczos3)
    };

    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    let encoded = encode(&image, target, args.quality)?;

    let dir = match args.output_dir.as_deref().filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => source.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let output = output_path(source, &dir, &args.suffix, target.extension());
    std::fs::write(&output, &encoded).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", output.to_string_lossy().as_ref())
            .with("detail", e.to_string())
    })?;

    Ok(Converted {
        output,
        target,
        after_bytes: encoded.len() as u64,
        size,
        new_size,
        thumbnail: with_thumbnail.then(|| thumbnail(&image)).flatten(),
    })
}

/// 批量转换：逐个解码 → 缩放 → 编码 → 写入输出目录。
/// 单个文件失败不影响其他文件，每个文件输出实时日志，按文件数汇报进度。
pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    if args.paths.is_empty() {
        return Err(PluginError::new("image.no_files"));
    }
    if let Some(dir) = args.output_dir.as_deref().filter(|d| !d.is_empty())
        && !Path::new(dir).is_dir()
    {
        return Err(PluginError::new("image.output_dir_missing").with("path", dir));
    }
    let start = Instant::now();
    let count = args.paths.len();
    let mut files = Vec::with_capacity(count);
    let (mut total_before, mut total_after, mut succeeded) = (0u64, 0u64, 0usize);

    ctx.stage("image.convert");
    for (index, raw) in args.paths.iter().enumerate() {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let source = Path::new(raw);
        let name = file_name(source);
        let before_bytes = ctx.file_size(raw).unwrap_or(0);
        ctx.log(
            LogLevel::Info,
            "image.file_start",
            json!({ "file": name, "index": index + 1, "count": count, "size": before_bytes }),
        );
        let file_start = Instant::now();

        match convert_one(source, &args, ctx, index < THUMB_LIMIT) {
            Ok(done) => {
                succeeded += 1;
                total_before += before_bytes;
                total_after += done.after_bytes;
                ctx.log(
                    LogLevel::Info,
                    "image.file_done",
                    json!({
                        "file": name,
                        "output": file_name(&done.output),
                        "width": done.new_size.0,
                        "height": done.new_size.1,
                        "before": before_bytes,
                        "after": done.after_bytes,
                        "ms": file_start.elapsed().as_millis() as u64,
                    }),
                );
                files.push(FileResult {
                    path: raw.clone(),
                    name,
                    output: Some(done.output.to_string_lossy().into_owned()),
                    format: Some(done.target),
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
                    "image.file_failed",
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
        total_before,
        total_after,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
