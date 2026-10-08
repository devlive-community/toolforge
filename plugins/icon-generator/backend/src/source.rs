//! 读取源图：位图直接解码，SVG 用 resvg 按需要的尺寸渲染。

use std::path::Path;

use image::RgbaImage;
use resvg::{tiny_skia, usvg};
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug)]
pub struct Source {
    pub image: RgbaImage,
    pub svg: bool,
    /// 原始尺寸（SVG 为其声明的尺寸）
    pub width: u32,
    pub height: u32,
}

fn is_svg(path: &Path, bytes: &[u8]) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        || bytes.starts_with(b"<svg")
        || (bytes.starts_with(b"<?xml") && bytes.windows(4).take(1024).any(|w| w == b"<svg"))
}

fn render_svg(bytes: &[u8], longest: u32) -> PluginResult<Source> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|e| PluginError::new("icon.invalid_svg").with("detail", e.to_string()))?;
    let size = tree.size();
    let scale = longest as f32 / size.width().max(size.height());
    let (w, h) = (
        (size.width() * scale).round().max(1.0) as u32,
        (size.height() * scale).round().max(1.0) as u32,
    );
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| PluginError::new("icon.invalid_svg").with("detail", "empty size"))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut image = RgbaImage::new(w, h);
    for (pixel, out) in pixmap.pixels().iter().zip(image.pixels_mut()) {
        let c = pixel.demultiply();
        *out = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(Source {
        image,
        svg: true,
        width: size.width().round() as u32,
        height: size.height().round() as u32,
    })
}

/// 读取源图；SVG 渲染时最长边为 `longest`
pub fn load(path: &str, longest: u32) -> PluginResult<Source> {
    let path = Path::new(path);
    let bytes = std::fs::read(path).map_err(|_| {
        PluginError::new("fs.not_found").with("path", path.to_string_lossy().as_ref())
    })?;
    if is_svg(path, &bytes) {
        return render_svg(&bytes, longest);
    }
    let image = image::load_from_memory(&bytes)
        .map_err(|e| PluginError::new("icon.unsupported_image").with("detail", e.to_string()))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Source {
        image,
        svg: false,
        width,
        height,
    })
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
