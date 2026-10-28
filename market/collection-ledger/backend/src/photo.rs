//! 藏品照片：读取用户选择的图片，按 EXIF 方向摆正、透明处铺白，
//! 压缩为 JPEG data URI（不保留原文件路径与元数据），大小在备份预算之内。

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbImage};
use tf_plugin_api::PluginResult;

use crate::backup::MAX_PHOTO;
use crate::model::error;

/// 源文件上限
pub const MAX_SOURCE: u64 = 16 * 1024 * 1024;
const MAX_SIDE: u32 = 8000;
const MAX_PIXELS: u64 = 24_000_000;
/// 依次尝试的最长边，直到压缩结果放得进预算
const EDGES: [u32; 5] = [640, 512, 384, 320, 256];

pub fn compress(bytes: &[u8]) -> PluginResult<String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| error("bad_photo"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| error("bad_photo"))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(error("photo_too_large"));
    }
    let orientation = decoder.orientation().map_err(|_| error("bad_photo"))?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| error("bad_photo"))?;
    image.apply_orientation(orientation);
    // 透明像素铺白，JPEG 不支持透明
    let rgba = image.into_rgba8();
    let mut flat = RgbImage::new(rgba.width(), rgba.height());
    for (dst, src) in flat.pixels_mut().zip(rgba.pixels()) {
        let a = u16::from(src[3]);
        for c in 0..3 {
            dst[c] = ((u16::from(src[c]) * a + 255 * (255 - a) + 127) / 255) as u8;
        }
    }
    let source = DynamicImage::ImageRgb8(flat);
    for edge in EDGES {
        // 只缩小，不放大小图
        let image = if source.width() > edge || source.height() > edge {
            source.thumbnail(edge, edge)
        } else {
            source.clone()
        };
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 78)
            .encode_image(&image)
            .map_err(|_| error("bad_photo"))?;
        let uri = format!(
            "data:image/jpeg;base64,{}",
            data_encoding::BASE64.encode(&out)
        );
        if uri.len() <= MAX_PHOTO {
            return Ok(uri);
        }
    }
    Err(error("photo_too_large"))
}

#[cfg(test)]
#[path = "photo_test.rs"]
mod tests;
