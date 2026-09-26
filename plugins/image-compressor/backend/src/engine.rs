//! 压缩引擎：JPEG 用 mozjpeg，PNG 用调色板量化（有损）或 oxipng（无损），WebP 用 libwebp。
//! ICC 色彩配置始终保留，EXIF 仅在需要时保留。

use std::borrow::Cow;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use exoquant::{Color, ditherer, optimizer};
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbImage};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// 保持原格式
    #[default]
    Keep,
    Jpeg,
    Png,
    Webp,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Keep | Self::Png => "png",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Keep | Self::Png => "image/png",
        }
    }

    fn from_image(format: ImageFormat) -> Option<Self> {
        match format {
            ImageFormat::Jpeg => Some(Self::Jpeg),
            ImageFormat::Png => Some(Self::Png),
            ImageFormat::WebP => Some(Self::Webp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PngMode {
    /// 量化为调色板图像，体积通常减少 60% 以上
    #[default]
    Lossy,
    /// 仅重新压缩，像素完全不变
    Lossless,
}

fn default_quality() -> u8 {
    75
}

fn default_colors() -> u16 {
    256
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    #[serde(default)]
    pub format: Format,
    /// JPEG / WebP 质量 1-100
    #[serde(default = "default_quality")]
    pub quality: u8,
    #[serde(default)]
    pub png_mode: PngMode,
    /// 有损 PNG 的最大颜色数 2-256
    #[serde(default = "default_colors")]
    pub colors: u16,
    #[serde(default = "yes")]
    pub dither: bool,
    /// 保留 EXIF 等元数据（拍摄信息、GPS）；ICC 色彩配置始终保留
    #[serde(default)]
    pub keep_metadata: bool,
    /// 长边上限，0 表示不缩放；只缩小不放大
    #[serde(default)]
    pub max_size: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: Format::Keep,
            quality: default_quality(),
            png_mode: PngMode::Lossy,
            colors: default_colors(),
            dither: true,
            keep_metadata: false,
            max_size: 0,
        }
    }
}

pub struct Source {
    pub format: Format,
    pub image: DynamicImage,
    pub icc: Option<Vec<u8>>,
    pub exif: Option<Vec<u8>>,
    /// 像素尚未按 EXIF 方向旋转（保留 EXIF 时由查看器负责旋转）
    pub orientation: Orientation,
}

fn decode_failed(err: impl ToString) -> PluginError {
    PluginError::new("image.decode_failed").with("detail", err.to_string())
}

fn encode_failed(err: impl ToString) -> PluginError {
    PluginError::new("image.encode_failed").with("detail", err.to_string())
}

/// 只识别格式，不解码像素
pub fn decode_format(bytes: &[u8]) -> PluginResult<Format> {
    image::guess_format(bytes)
        .ok()
        .and_then(Format::from_image)
        .ok_or_else(|| PluginError::new("image.unsupported"))
}

pub fn decode(bytes: &[u8]) -> PluginResult<Source> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(decode_failed)?;
    let format = reader
        .format()
        .and_then(Format::from_image)
        .ok_or_else(|| PluginError::new("image.unsupported"))?;
    let mut decoder = reader.into_decoder().map_err(decode_failed)?;
    let icc = decoder
        .icc_profile()
        .ok()
        .flatten()
        .filter(|p| !p.is_empty());
    let exif = decoder
        .exif_metadata()
        .ok()
        .flatten()
        .filter(|e| !e.is_empty());
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let image = DynamicImage::from_decoder(decoder).map_err(decode_failed)?;
    Ok(Source {
        format,
        image,
        icc,
        exif,
        orientation,
    })
}

/// 长边不超过 max 时的新尺寸
pub fn fit(width: u32, height: u32, max: u32) -> (u32, u32) {
    let long = width.max(height);
    if max == 0 || long <= max {
        return (width, height);
    }
    let ratio = max as f64 / long as f64;
    let scale = |v: u32| ((v as f64 * ratio).round() as u32).max(1);
    (scale(width), scale(height))
}

/// JPEG 不支持透明：合成到白色背景上
fn flatten(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |c: u8| ((c as u32 * a as u32 + 255 * (255 - a as u32)) / 255) as u8;
        image::Rgb([blend(r), blend(g), blend(b)])
    })
}

fn is_gray(image: &DynamicImage) -> bool {
    matches!(
        image,
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageLuma16(_)
    )
}

pub fn encode_jpeg(
    image: &DynamicImage,
    quality: u8,
    icc: Option<&[u8]>,
    exif: Option<&[u8]>,
) -> PluginResult<Vec<u8>> {
    let gray = is_gray(image);
    let (pixels, width, height) = if gray {
        let luma = image.to_luma8();
        let (w, h) = luma.dimensions();
        (luma.into_raw(), w, h)
    } else {
        let rgb = flatten(image);
        let (w, h) = rgb.dimensions();
        (rgb.into_raw(), w, h)
    };
    // libjpeg 的错误以 panic 形式抛出
    catch_unwind(AssertUnwindSafe(|| -> std::io::Result<Vec<u8>> {
        let space = if gray {
            mozjpeg::ColorSpace::JCS_GRAYSCALE
        } else {
            mozjpeg::ColorSpace::JCS_RGB
        };
        let mut compress = mozjpeg::Compress::new(space);
        compress.set_size(width as usize, height as usize);
        compress.set_quality(quality.clamp(1, 100) as f32);
        compress.set_progressive_mode();
        compress.set_optimize_scans(true);
        compress.set_optimize_coding(true);
        let mut started = compress.start_compress(Vec::new())?;
        if let Some(exif) = exif {
            let mut app1 = Vec::with_capacity(exif.len() + 6);
            if !exif.starts_with(b"Exif\0\0") {
                app1.extend_from_slice(b"Exif\0\0");
            }
            app1.extend_from_slice(exif);
            if app1.len() <= 65_533 {
                started.write_marker(mozjpeg::Marker::APP(1), &app1);
            }
        }
        for marker in icc.map(icc_markers).unwrap_or_default() {
            started.write_marker(mozjpeg::Marker::APP(2), &marker);
        }
        started.write_scanlines(&pixels)?;
        started.finish()
    }))
    .map_err(|_| encode_failed("libjpeg error"))?
    .map_err(encode_failed)
}

/// 把 ICC 配置拆成 APP2 段。序号必须从 1 开始：mozjpeg 自带的
/// `write_icc_profile` 从 0 开始编号，解码器会因此丢弃整个配置。
pub fn icc_markers(icc: &[u8]) -> Vec<Vec<u8>> {
    const MAX_DATA: usize = 65_533 - 14;
    let chunks: Vec<&[u8]> = icc.chunks(MAX_DATA).collect();
    if chunks.is_empty() || chunks.len() > 255 {
        return Vec::new();
    }
    let count = chunks.len() as u8;
    chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let mut marker = Vec::with_capacity(chunk.len() + 14);
            marker.extend_from_slice(b"ICC_PROFILE\0");
            marker.extend([index as u8 + 1, count]);
            marker.extend_from_slice(chunk);
            marker
        })
        .collect()
}

/// 量化为不超过 colors 种颜色（含透明度）的调色板 PNG
pub fn encode_png_lossy(
    image: &DynamicImage,
    colors: u16,
    dither: bool,
    icc: Option<&[u8]>,
) -> PluginResult<Vec<u8>> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels: Vec<Color> = rgba
        .pixels()
        .map(|p| Color::new(p[0], p[1], p[2], p[3]))
        .collect();
    let (mut palette, indexed) = quantize(
        &pixels,
        width as usize,
        colors.clamp(2, 256) as usize,
        dither,
    );
    // k-means 在浮点空间平均透明度，会把 255 舍入成 254，让不透明图片带上多余的 tRNS
    let opaque = pixels.iter().all(|p| p.a == 255);
    for color in &mut palette {
        color.a = match color.a {
            _ if opaque => 255,
            253.. => 255,
            0..=2 => 0,
            a => a,
        };
    }
    let depth = match palette.len() {
        0..=2 => png::BitDepth::One,
        3..=4 => png::BitDepth::Two,
        5..=16 => png::BitDepth::Four,
        _ => png::BitDepth::Eight,
    };
    let data = pack(&indexed, width as usize, depth);
    let rgb: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
    // tRNS 只需要写到最后一个非不透明的颜色
    let alpha: Vec<u8> = palette.iter().map(|c| c.a).collect();
    let trns_len = alpha.iter().rposition(|a| *a != 255).map_or(0, |i| i + 1);

    let mut info = png::Info::with_size(width, height);
    info.color_type = png::ColorType::Indexed;
    info.bit_depth = depth;
    info.palette = Some(Cow::Owned(rgb));
    if trns_len > 0 {
        info.trns = Some(Cow::Owned(alpha[..trns_len].to_vec()));
    }
    info.icc_profile = icc.map(|p| Cow::Owned(p.to_vec()));
    let mut buffer = Vec::new();
    {
        let encoder = png::Encoder::with_info(&mut buffer, info).map_err(encode_failed)?;
        let mut writer = encoder.write_header().map_err(encode_failed)?;
        writer.write_image_data(&data).map_err(encode_failed)?;
        writer.finish().map_err(encode_failed)?;
    }
    optimize_png(&buffer, true)
}

/// 生成调色板时最多采样的像素数：颜色分布足够准确，大图也不会太慢
const PALETTE_SAMPLES: usize = 256 * 1024;
/// 并行抖动时每条水平带至少的行数
const BAND_ROWS: usize = 64;

/// 采样像素生成并用 k-means 优化调色板，再按水平带并行映射（可选 Floyd–Steinberg 抖动）
fn quantize(pixels: &[Color], width: usize, colors: usize, dither: bool) -> (Vec<Color>, Vec<u8>) {
    let step = pixels.len().div_ceil(PALETTE_SAMPLES).max(1);
    let histogram: exoquant::Histogram = pixels.iter().step_by(step).cloned().collect();
    let space = exoquant::SimpleColorSpace::default();
    let palette = exoquant::generate_palette(&histogram, &space, &optimizer::KMeans, colors);
    let palette =
        optimizer::Optimizer::optimize_palette(&optimizer::KMeans, &space, &palette, &histogram, 8);

    // 按 CPU 数切成连续的水平带，带与带之间的误差扩散互不影响，接缝不可见
    let rows = pixels.len() / width.max(1);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let bands = threads.min(rows / BAND_ROWS).max(1);
    let band = width.max(1) * rows.div_ceil(bands).max(1);
    let bands: Vec<Vec<u8>> = std::thread::scope(|scope| {
        let workers: Vec<_> = pixels
            .chunks(band)
            .map(|chunk| {
                let (palette, space) = (&palette, &space);
                scope.spawn(move || {
                    if dither {
                        exoquant::Remapper::new(palette, space, &ditherer::FloydSteinberg::new())
                            .remap(chunk, width)
                    } else {
                        exoquant::Remapper::new(palette, space, &ditherer::None).remap(chunk, width)
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap_or_default())
            .collect()
    });
    let indexed: Vec<u8> = bands.concat();
    exoquant::sort_palette(&palette, &indexed)
}

/// 按位深把每行的调色板索引打包
fn pack(indexed: &[u8], width: usize, depth: png::BitDepth) -> Vec<u8> {
    let bits = depth as usize;
    if bits == 8 {
        return indexed.to_vec();
    }
    let per_byte = 8 / bits;
    let row_bytes = width.div_ceil(per_byte);
    let mut out = Vec::with_capacity(row_bytes * indexed.len() / width.max(1));
    for row in indexed.chunks(width) {
        for group in row.chunks(per_byte) {
            let mut byte = 0u8;
            for (i, index) in group.iter().enumerate() {
                byte |= index << (8 - bits * (i + 1));
            }
            out.push(byte);
        }
    }
    out
}

/// oxipng 无损优化；strip 为 true 时去掉不影响显示的元数据块
pub fn optimize_png(bytes: &[u8], strip: bool) -> PluginResult<Vec<u8>> {
    let mut options = oxipng::Options::from_preset(2);
    options.strip = if strip {
        oxipng::StripChunks::Safe
    } else {
        oxipng::StripChunks::None
    };
    oxipng::optimize_from_memory(bytes, &options).map_err(encode_failed)
}

pub fn encode_png_lossless(image: &DynamicImage, icc: Option<&[u8]>) -> PluginResult<Vec<u8>> {
    let image = if image.color().has_alpha() {
        DynamicImage::ImageRgba8(image.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
    };
    let mut info = png::Info::with_size(image.width(), image.height());
    info.color_type = if image.color().has_alpha() {
        png::ColorType::Rgba
    } else {
        png::ColorType::Rgb
    };
    info.bit_depth = png::BitDepth::Eight;
    info.icc_profile = icc.map(|p| Cow::Owned(p.to_vec()));
    let mut buffer = Vec::new();
    {
        let encoder = png::Encoder::with_info(&mut buffer, info).map_err(encode_failed)?;
        let mut writer = encoder.write_header().map_err(encode_failed)?;
        writer
            .write_image_data(image.as_bytes())
            .map_err(encode_failed)?;
        writer.finish().map_err(encode_failed)?;
    }
    optimize_png(&buffer, true)
}

pub fn encode_webp(image: &DynamicImage, quality: u8) -> PluginResult<Vec<u8>> {
    let memory = if image.color().has_alpha() {
        let rgba = image.to_rgba8();
        webp::Encoder::from_rgba(&rgba, rgba.width(), rgba.height()).encode(quality as f32)
    } else {
        let rgb = image.to_rgb8();
        webp::Encoder::from_rgb(&rgb, rgb.width(), rgb.height()).encode(quality as f32)
    };
    if memory.is_empty() {
        return Err(encode_failed("libwebp error"));
    }
    Ok(memory.to_vec())
}

#[derive(Debug)]
pub struct Compressed {
    pub format: Format,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 压缩一张图片（已读入内存的原始字节）
pub fn compress(original: &[u8], options: &Options) -> PluginResult<Compressed> {
    let source = decode(original)?;
    let format = match options.format {
        Format::Keep => source.format,
        other => other,
    };
    // 仅在输出 JPEG 且保留 EXIF 时保持原始方向，其余情况把像素摆正
    let keep_exif = options.keep_metadata && format == Format::Jpeg && source.exif.is_some();
    let mut image = source.image;
    if !keep_exif {
        image.apply_orientation(source.orientation);
    }
    let (width, height) = fit(image.width(), image.height(), options.max_size);
    let resized = (width, height) != (image.width(), image.height());
    if resized {
        image = image.resize_exact(width, height, FilterType::Lanczos3);
    }
    let icc = source.icc.as_deref();
    let bytes = match format {
        Format::Jpeg => encode_jpeg(
            &image,
            options.quality,
            icc,
            source.exif.as_deref().filter(|_| keep_exif),
        )?,
        Format::Webp => encode_webp(&image, options.quality)?,
        Format::Png | Format::Keep => match options.png_mode {
            PngMode::Lossy => encode_png_lossy(&image, options.colors, options.dither, icc)?,
            // 源文件就是 PNG 且未缩放时直接优化原始数据，保证像素与块完全不变
            PngMode::Lossless if source.format == Format::Png && !resized => {
                optimize_png(original, !options.keep_metadata)?
            }
            PngMode::Lossless => encode_png_lossless(&image, icc)?,
        },
    };
    Ok(Compressed {
        format,
        bytes,
        width,
        height,
    })
}

#[cfg(test)]
#[path = "engine_test.rs"]
mod tests;
