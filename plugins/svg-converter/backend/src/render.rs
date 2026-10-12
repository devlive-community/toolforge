//! SVG 解析与栅格化：resvg 渲染到目标尺寸，系统字体进程内只加载一次。

use std::sync::{Arc, OnceLock};

use image::RgbaImage;
use resvg::{tiny_skia, usvg};
use tf_plugin_api::{PluginError, PluginResult};

/// 含文本的 SVG 需要字体才能正确渲染；加载系统字体开销大，进程内只做一次
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// 解析 SVG；失败的常见原因是文件不是 SVG 或包含不支持的特性
pub fn parse(bytes: &[u8]) -> PluginResult<usvg::Tree> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    usvg::Tree::from_data(bytes, &options)
        .map_err(|e| PluginError::new("svg.invalid").with("detail", format!("{e:?}")))
}

/// 把树渲染为指定像素尺寸；宽高可各自独立指定（与声明尺寸不成比例时分别缩放）
pub fn render(tree: &usvg::Tree, width: u32, height: u32) -> PluginResult<RgbaImage> {
    let size = tree.size();
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| PluginError::new("svg.render_failed").with("detail", "empty pixmap"))?;
    let sx = width as f32 / size.width();
    let sy = height as f32 / size.height();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    // resvg 输出预乘 alpha，转为常规 RGBA
    let mut image = RgbaImage::new(width, height);
    for (pixel, out) in pixmap.pixels().iter().zip(image.pixels_mut()) {
        let c = pixel.demultiply();
        *out = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(image)
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
