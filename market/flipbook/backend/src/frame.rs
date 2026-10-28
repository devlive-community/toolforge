//! 帧数据：画布尺寸与上限、帧像素与存档文本（PNG 的 base64）之间的转换。
//!
//! - 空字符串表示全透明的空白帧：新增空白帧不占存储，也不需要写入状态键。
//! - 一帧编码后的文本不超过 `MAX_FRAME_TEXT`，保证单个状态键低于宿主 2 MiB 上限。
//! - 解码时校验尺寸必须与作品画布一致，避免不同帧尺寸混在一起。

use std::io::Cursor;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder, ImageFormat, RgbaImage};
use tf_plugin_api::{PluginError, PluginResult};

/// 允许的画布尺寸（宽, 高）
pub const SIZES: [(u32, u32); 6] = [
    (240, 180),
    (320, 240),
    (480, 360),
    (640, 480),
    (320, 320),
    (480, 480),
];
/// 每个作品的帧数上限
pub const MAX_FRAMES: usize = 120;
/// 单帧存档文本上限（base64 字符数）
pub const MAX_FRAME_TEXT: usize = 1_800_000;
/// 单个作品全部帧存档文本合计上限
pub const MAX_PROJECT_TEXT: u64 = 40 * 1024 * 1024;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("flipbook.{code}"))
}

/// 画布尺寸必须是预设之一
pub fn check_size(width: u32, height: u32) -> PluginResult<()> {
    if SIZES.contains(&(width, height)) {
        Ok(())
    } else {
        Err(error("bad_size")
            .with("width", width)
            .with("height", height))
    }
}

/// 存档文本 → 像素；空文本为透明画布
pub fn decode(text: &str, width: u32, height: u32) -> PluginResult<RgbaImage> {
    check_size(width, height)?;
    if text.is_empty() {
        return Ok(RgbaImage::new(width, height));
    }
    if text.len() > MAX_FRAME_TEXT {
        return Err(error("frame_too_large"));
    }
    let bytes = data_encoding::BASE64
        .decode(text.as_bytes())
        .map_err(|_| error("bad_frame"))?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|_| error("bad_frame"))?
        .into_rgba8();
    if image.dimensions() != (width, height) {
        return Err(error("bad_frame"));
    }
    Ok(image)
}

/// 像素 → 存档文本；全透明时返回空文本
pub fn encode(image: &RgbaImage) -> PluginResult<String> {
    if image.pixels().all(|p| p[3] == 0) {
        return Ok(String::new());
    }
    let bytes = png_bytes(image)?;
    let text = data_encoding::BASE64.encode(&bytes);
    if text.len() > MAX_FRAME_TEXT {
        return Err(error("frame_too_large"));
    }
    Ok(text)
}

/// 编码为 PNG 字节（保留透明度）
pub fn png_bytes(image: &RgbaImage) -> PluginResult<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    PngEncoder::new_with_quality(&mut out, CompressionType::Best, FilterType::Adaptive)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| error("encode"))?;
    Ok(out.into_inner())
}

/// `#rrggbb` → RGB
pub fn parse_color(text: &str) -> PluginResult<[u8; 3]> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(error("bad_color").with("color", text));
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Ok([channel(0), channel(2), channel(4)])
}

/// 源像素按 alpha 叠加到目标像素上（source-over）
pub fn blend(dst: &mut image::Rgba<u8>, src: [u8; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let da = f32::from(dst[3]) / 255.0;
    let out_a = alpha + da * (1.0 - alpha);
    if out_a <= 0.0 {
        *dst = image::Rgba([0, 0, 0, 0]);
        return;
    }
    for i in 0..3 {
        let s = f32::from(src[i]);
        let d = f32::from(dst[i]);
        let v = (s * alpha + d * da * (1.0 - alpha)) / out_a;
        dst[i] = v.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

#[cfg(test)]
#[path = "frame_test.rs"]
mod tests;
