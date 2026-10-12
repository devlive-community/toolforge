//! 拼接逻辑：解码全部输入 → 计算画布与位置 → 贴图 → 编码输出。

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::metadata::Orientation;
use image::{ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader, Rgb, RgbImage, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

const MAX_FILE: u64 = 200 * 1024 * 1024;
/// 单次拼接的图片数量上限
const MAX_IMAGES: usize = 100;
/// 画布像素预算
const MAX_PIXELS: u64 = 40_000_000;
const MAX_GAP: u32 = 1000;
const MAX_COLUMNS: u32 = 64;
/// 结果预览图的最长边
const THUMB: u32 = 480;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// 从左到右排成一行
    #[default]
    Horizontal,
    /// 从上到下排成一列
    Vertical,
    /// 网格；列数由 columns 指定，行数自动，每格居中
    Grid,
}

/// 非网格布局的对齐：水平布局指垂直方向（上 / 中 / 下），垂直布局指水平方向（左 / 中 / 右）
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Background {
    #[default]
    Transparent,
    White,
    Black,
    /// 自定义颜色（#RRGGBB 或 #RRGGBBAA）
    Color {
        hex: String,
    },
}

impl Background {
    /// RGBA 颜色；透明为全 0
    pub fn rgba(&self) -> PluginResult<[u8; 4]> {
        let hex = match self {
            Self::Transparent => return Ok([0, 0, 0, 0]),
            Self::White => return Ok([255, 255, 255, 255]),
            Self::Black => return Ok([0, 0, 0, 255]),
            Self::Color { hex } => hex.trim().trim_start_matches('#'),
        };
        if !matches!(hex.len(), 6 | 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(PluginError::new("stitch.invalid_color").with("color", hex));
        }
        let v = u32::from_str_radix(hex, 16)
            .map_err(|_| PluginError::new("stitch.invalid_color").with("color", hex))?;
        let [r, g, b, a] = if hex.len() == 6 {
            [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
        } else {
            [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8]
        };
        Ok([r, g, b, a])
    }

    pub fn is_transparent(&self) -> bool {
        matches!(self, Self::Transparent)
    }
}

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

fn default_columns() -> u32 {
    2
}

fn default_quality() -> u8 {
    85
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub paths: Vec<String>,
    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub align: Align,
    /// 网格布局的列数
    #[serde(default = "default_columns")]
    pub columns: u32,
    /// 图与图之间的间距（像素）
    #[serde(default)]
    pub gap: u32,
    #[serde(default)]
    pub background: Background,
    #[serde(default)]
    pub format: Format,
    /// JPEG 质量 1-100
    #[serde(default = "default_quality")]
    pub quality: u8,
    /// 输出目录；为空时输出到第一张图所在目录
    #[serde(default)]
    pub output_dir: Option<String>,
    /// 输出文件名（不含扩展名），为空时用 stitched
    #[serde(default)]
    pub name: Option<String>,
}

/// 画布尺寸与每张图的左上角位置
#[derive(Debug)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub placements: Vec<(u32, u32)>,
}

/// 按布局计算画布与每张图的位置；总像素超过预算时报错
pub fn layout(sizes: &[(u32, u32)], args: &Args) -> PluginResult<Canvas> {
    let gap = args.gap.min(MAX_GAP) as u64;
    let n = sizes.len() as u64;
    let place = |offset: u64, size: u32, total: u64, align: Align| -> u64 {
        match align {
            Align::Start => offset,
            Align::Center => offset + (total - size as u64) / 2,
            Align::End => offset + total - size as u64,
        }
    };
    let (width, height, placements) = match args.layout {
        Layout::Horizontal => {
            let width: u64 = sizes.iter().map(|s| s.0 as u64).sum::<u64>() + gap * (n - 1);
            let height = sizes.iter().map(|s| s.1 as u64).max().unwrap_or(0);
            let mut x = 0u64;
            let placements = sizes
                .iter()
                .map(|s| {
                    let p = (x, place(0, s.1, height, args.align));
                    x += s.0 as u64 + gap;
                    p
                })
                .collect();
            (width, height, placements)
        }
        Layout::Vertical => {
            let width = sizes.iter().map(|s| s.0 as u64).max().unwrap_or(0);
            let height: u64 = sizes.iter().map(|s| s.1 as u64).sum::<u64>() + gap * (n - 1);
            let mut y = 0u64;
            let placements = sizes
                .iter()
                .map(|s| {
                    let p = (place(0, s.0, width, args.align), y);
                    y += s.1 as u64 + gap;
                    p
                })
                .collect();
            (width, height, placements)
        }
        Layout::Grid => {
            let columns = args
                .columns
                .clamp(1, MAX_COLUMNS)
                .min(sizes.len().max(1) as u32) as usize;
            let rows = sizes.len().div_ceil(columns);
            // 每列宽 = 该列最大图宽，每行高 = 该行最大图高
            let mut col_width = vec![0u64; columns];
            let mut row_height = vec![0u64; rows];
            for (index, s) in sizes.iter().enumerate() {
                let (row, col) = (index / columns, index % columns);
                col_width[col] = col_width[col].max(s.0 as u64);
                row_height[row] = row_height[row].max(s.1 as u64);
            }
            let width: u64 = col_width.iter().sum::<u64>() + gap * (columns as u64 - 1);
            let height: u64 = row_height.iter().sum::<u64>() + gap * (rows as u64 - 1);
            let mut x = 0u64;
            let mut col = 0usize;
            let mut row = 0usize;
            let placements: Vec<(u64, u64)> = sizes
                .iter()
                .map(|s| {
                    if col == columns {
                        col = 0;
                        row += 1;
                        x = 0;
                    }
                    let p = (
                        x + (col_width[col] - s.0 as u64) / 2,
                        row_height[..row].iter().sum::<u64>()
                            + gap * row as u64
                            + (row_height[row] - s.1 as u64) / 2,
                    );
                    x += col_width[col] + gap;
                    col += 1;
                    p
                })
                .collect();
            (width, height, placements)
        }
    };
    // 先各自检查再相乘，避免乘积溢出
    if width > u32::MAX as u64 || height > u32::MAX as u64 || width * height > MAX_PIXELS {
        return Err(PluginError::new("stitch.pixels_too_large")
            .with("width", width.min(u32::MAX as u64))
            .with("height", height.min(u32::MAX as u64)));
    }
    let placements = placements
        .into_iter()
        .map(|(x, y)| (x as u32, y as u32))
        .collect();
    Ok(Canvas {
        width: width as u32,
        height: height as u32,
        placements,
    })
}

/// 解码为 RGBA8，并按 EXIF 方向摆正
pub fn decode(bytes: &[u8]) -> PluginResult<RgbaImage> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PluginError::new("stitch.decode_failed").with("detail", e.to_string()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| PluginError::new("stitch.decode_failed").with("detail", e.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| PluginError::new("stitch.decode_failed").with("detail", e.to_string()))?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

fn encode_failed(err: impl ToString) -> PluginError {
    PluginError::new("stitch.encode_failed").with("detail", err.to_string())
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
        // JPEG 无透明；画布在 JPEG 输出时总是不透明背景
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

/// 去掉 alpha 通道（画布已经合成过背景，像素全部不透明）
fn flatten(image: &RgbaImage) -> RgbImage {
    RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, _] = image.get_pixel(x, y).0;
        Rgb([r, g, b])
    })
}

/// 在输出目录中生成不与已有文件冲突的路径
pub fn output_path(dir: &Path, name: &str, extension: &str) -> PathBuf {
    let stem = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "stitched".to_owned());
    let mut candidate = dir.join(format!("{stem}.{extension}"));
    let mut index = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({index}).{extension}"));
        index += 1;
    }
    candidate
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub elapsed_ms: u64,
    /// 结果预览图（PNG Data URI，最长边 480）
    pub thumbnail: String,
}

/// 拼接：逐个解码（按文件进度）→ 布局 → 贴图 → 编码 → 写盘。
pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    if args.paths.len() < 2 {
        return Err(PluginError::new("stitch.too_few"));
    }
    if args.paths.len() > MAX_IMAGES {
        return Err(PluginError::new("stitch.too_many").with("limit", MAX_IMAGES as u64));
    }
    if let Some(dir) = args.output_dir.as_deref().filter(|d| !d.is_empty())
        && !Path::new(dir).is_dir()
    {
        return Err(PluginError::new("stitch.output_dir_missing").with("path", dir));
    }
    let start = Instant::now();
    let count = args.paths.len();
    // JPEG 输出没有透明：透明背景回退为白色
    let background = if args.format == Format::Jpeg && args.background.is_transparent() {
        Background::White
    } else {
        args.background.clone()
    };

    ctx.stage("stitch.decode");
    let mut images = Vec::with_capacity(count);
    for (index, path) in args.paths.iter().enumerate() {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        if ctx.file_size(path)? > MAX_FILE {
            return Err(PluginError::new("stitch.file_too_large").with("limit", "200 MB"));
        }
        let mut bytes = Vec::new();
        ctx.open_file(path)?.read_to_end(&mut bytes).map_err(|e| {
            PluginError::new("stitch.io")
                .with("path", path.as_str())
                .with("detail", e.to_string())
        })?;
        let image = decode(&bytes)?;
        ctx.log(
            LogLevel::Info,
            "stitch.image_loaded",
            json!({
                "file": name,
                "index": index + 1,
                "count": count,
                "width": image.width(),
                "height": image.height(),
            }),
        );
        ctx.progress(index as u64 + 1, count as u64 + 2);
        images.push(image);
    }

    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    ctx.stage("stitch.compose");
    let sizes = images.iter().map(|i| i.dimensions()).collect::<Vec<_>>();
    let canvas = layout(&sizes, &args)?;
    let background_color = background.rgba()?;
    let mut result =
        RgbaImage::from_pixel(canvas.width, canvas.height, image::Rgba(background_color));
    for (image, (x, y)) in images.iter().zip(&canvas.placements) {
        // overlay 做 alpha 合成：不透明背景上贴半透明图得到正确的混合结果
        image::imageops::overlay(&mut result, image, *x as i64, *y as i64);
    }
    ctx.progress(count as u64 + 1, count as u64 + 2);

    ctx.stage("stitch.encode");
    let encoded = encode(&result, args.format, args.quality)?;
    let dir = match args.output_dir.as_deref().filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(&args.paths[0])
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
    };
    let output = output_path(
        &dir,
        args.name.as_deref().unwrap_or("stitched"),
        args.format.extension(),
    );
    std::fs::write(&output, &encoded).map_err(|e| {
        PluginError::new("stitch.io")
            .with("path", output.to_string_lossy().as_ref())
            .with("detail", e.to_string())
    })?;

    // imageops::thumbnail 返回的就是 RGBA8 的 ImageBuffer，直接按 PNG 编码
    let thumbnail = encode(
        &image::imageops::thumbnail(&result, THUMB, THUMB),
        Format::Png,
        85,
    )?;

    ctx.log(
        LogLevel::Info,
        "stitch.done",
        json!({
            "output": output.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            "count": count,
            "width": canvas.width,
            "height": canvas.height,
            "bytes": encoded.len(),
            "ms": start.elapsed().as_millis() as u64,
        }),
    );
    ctx.progress(count as u64 + 2, count as u64 + 2);

    Ok(Report {
        output: output.to_string_lossy().into_owned(),
        width: canvas.width,
        height: canvas.height,
        bytes: encoded.len() as u64,
        elapsed_ms: start.elapsed().as_millis() as u64,
        thumbnail: format!(
            "data:image/png;base64,{}",
            data_encoding::BASE64.encode(&thumbnail)
        ),
    })
}

#[cfg(test)]
#[path = "stitch_test.rs"]
mod tests;
