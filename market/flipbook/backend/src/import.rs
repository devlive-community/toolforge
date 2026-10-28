//! 导入本地图片到当前帧：按 EXIF 方向摆正，等比缩放到画布内（contain）并居中，
//! 叠加在当前帧已有内容之上。原图路径与元数据都不保留。

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use tf_plugin_api::PluginResult;

use crate::frame::{blend, error};

/// 源文件大小上限
pub const MAX_FILE: u64 = 20 * 1024 * 1024;
/// 源图片单边与像素上限
const MAX_SIDE: u32 = 8000;
const MAX_PIXELS: u64 = 24_000_000;

/// 解码图片字节（PNG/JPEG/WebP/GIF 首帧），带尺寸与内存上限
pub fn decode(bytes: &[u8]) -> PluginResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| error("bad_image"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| error("bad_image"))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(error("image_too_large"));
    }
    let orientation = decoder.orientation().map_err(|_| error("bad_image"))?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| error("bad_image"))?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// 把图片等比缩放后居中叠加到帧上
pub fn place(frame: &mut RgbaImage, picture: &DynamicImage) {
    let (fw, fh) = frame.dimensions();
    let (pw, ph) = (picture.width(), picture.height());
    let scale = (f64::from(fw) / f64::from(pw)).min(f64::from(fh) / f64::from(ph));
    let tw = ((f64::from(pw) * scale).round() as u32).clamp(1, fw);
    let th = ((f64::from(ph) * scale).round() as u32).clamp(1, fh);
    let resized = picture
        .resize_exact(tw, th, FilterType::CatmullRom)
        .into_rgba8();
    let ox = (fw - tw) / 2;
    let oy = (fh - th) / 2;
    for (x, y, src) in resized.enumerate_pixels() {
        let alpha = f32::from(src[3]) / 255.0;
        blend(
            frame.get_pixel_mut(ox + x, oy + y),
            [src[0], src[1], src[2]],
            alpha,
        );
    }
}

#[cfg(test)]
#[path = "import_test.rs"]
mod tests;
