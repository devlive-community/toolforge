//! 照片加载与受限工作图：解码、像素上限、缩边、JPEG 重编码与指纹。

use std::hash::{Hash, Hasher};

use data_encoding::BASE64;
use image::{DynamicImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 工作图长边上限
pub const WORK_LONG_EDGE: u32 = 1600;
/// 源照片像素上限
pub const MAX_SOURCE_PIXELS: u64 = 100_000_000;
/// 源文件大小上限（仅路径读取时检查）
pub const MAX_FILE_BYTES: u64 = 40_000_000;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 输入来源：前端 Data URI 或用户选择的文件路径
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInput {
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

impl ImageInput {
    pub fn read_bytes(&self) -> PluginResult<Vec<u8>> {
        if let Some(data) = &self.data {
            let Some(encoded) = data.rsplit_once("base64,").map(|(_, tail)| tail) else {
                return Err(errp("jigsaw.bad_data", "hint", "data uri prefix"));
            };
            return BASE64
                .decode(encoded.as_bytes())
                .map_err(|e| errp("jigsaw.bad_data", "detail", e.to_string()));
        }
        let Some(path) = &self.path else {
            return Err(errp("jigsaw.source_required", "hint", ""));
        };
        let size = tf_plugin_guest::net::file_size(path).map_err(|e| {
            errp("jigsaw.file_unreadable", "path", path.as_str()).with("detail", e.to_string())
        })?;
        if size > MAX_FILE_BYTES {
            return Err(errp("jigsaw.file_too_large", "limit", "40 MB"));
        }
        tf_plugin_guest::net::read_file(path, 0, u32::try_from(size).unwrap_or(u32::MAX)).map_err(
            |e| errp("jigsaw.file_unreadable", "path", path.as_str()).with("detail", e.to_string()),
        )
    }
}

pub fn decode(bytes: &[u8]) -> PluginResult<DynamicImage> {
    let image = image::load_from_memory(bytes)
        .map_err(|e| errp("jigsaw.decode_failed", "detail", e.to_string()))?;
    if u64::from(image.width()) * u64::from(image.height()) > MAX_SOURCE_PIXELS {
        return Err(errp("jigsaw.pixels_too_large", "limit", "100 MP"));
    }
    Ok(image)
}

/// 透明区域按白底合成，避免 JPEG 直接丢 alpha 变黑
fn flatten_to_rgb(image: &DynamicImage) -> RgbaImage {
    let rgba = image.to_rgba8();
    let mut out = RgbaImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let a = f64::from(pixel[3]) / 255.0;
        let blend = |v: u8| ((f64::from(v) * a) + 255.0 * (1.0 - a)).round() as u8;
        out.put_pixel(
            x,
            y,
            Rgba([blend(pixel[0]), blend(pixel[1]), blend(pixel[2]), 255]),
        );
    }
    out
}

/// 缩到长边上限以内并编码为 JPEG（照片内容体积小、无 alpha）
pub fn work_image(image: &DynamicImage, long_edge: u32) -> PluginResult<Vec<u8>> {
    let flat = flatten_to_rgb(image);
    let scaled = if flat.width().max(flat.height()) > long_edge {
        DynamicImage::ImageRgba8(flat).thumbnail(long_edge, long_edge)
    } else {
        DynamicImage::ImageRgba8(flat)
    };
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85);
    encoder
        .encode_image(&image::RgbaImage::from(scaled))
        .map_err(|e| errp("jigsaw.encode_failed", "detail", e.to_string()))?;
    Ok(bytes)
}

pub fn to_data_uri(bytes: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", BASE64.encode(bytes))
}

pub fn decode_data_uri(data: &str) -> PluginResult<DynamicImage> {
    let encoded = data
        .rsplit_once("base64,")
        .map(|(_, tail)| tail)
        .ok_or_else(|| errp("jigsaw.bad_data", "hint", "data uri prefix"))?;
    let bytes = BASE64
        .decode(encoded.as_bytes())
        .map_err(|e| errp("jigsaw.bad_data", "detail", e.to_string()))?;
    decode(&bytes)
}

/// 照片指纹：原始字节哈希，用于区分不同照片的存档与记录
pub fn photo_key(bytes: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// 读取并压缩到受限工作图
pub fn prepare(input: &ImageInput) -> PluginResult<Prepared> {
    let bytes = input.read_bytes()?;
    let image = decode(&bytes)?;
    let key = photo_key(&bytes);
    let work = work_image(&image, WORK_LONG_EDGE)?;
    let sized = decode(&work)?;
    Ok(Prepared {
        work_data: to_data_uri(&work),
        width: sized.width(),
        height: sized.height(),
        photo_key: key,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    pub work_data: String,
    pub width: u32,
    pub height: u32,
    pub photo_key: String,
}

#[cfg(test)]
#[path = "prepare_test.rs"]
mod tests;
