//! SVG 解析与栅格化：resvg 渲染到目标尺寸；含文本时才加载字体，进程内只加载一次。

use std::sync::{Arc, OnceLock};

use image::RgbaImage;
use resvg::{tiny_skia, usvg};
use tf_plugin_api::{PluginError, PluginResult};
use tf_plugin_guest::net;

/// 单个字体文件的读取上限
const MAX_FONT: u64 = 64 * 1024 * 1024;

/// 各平台常见的字体文件，每类取第一个能读到的。WASM 沙箱不能遍历系统字体目录，
/// 只能按路径读取；命令行与 Agent 只能读授权目录，读不到时文本不渲染。
const SANS: &[&str] = &[
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
];
const SERIF: &[&str] = &[
    "/System/Library/Fonts/Times.ttc",
    "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
    "C:\\Windows\\Fonts\\times.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
    "/usr/share/fonts/dejavu/DejaVuSerif.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
];
const MONOSPACE: &[&str] = &[
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Supplemental/Courier New.ttf",
    "C:\\Windows\\Fonts\\consola.ttf",
    "C:\\Windows\\Fonts\\cour.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
];
/// 中文字体：其他字体缺字时由 usvg 回退到这里
const CJK: &[&str] = &[
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\simsun.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];

/// 读入候选中第一个存在的字体，返回它的字体族名
fn load_first(db: &mut usvg::fontdb::Database, candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|path| {
        let data = net::read_to_end(path, MAX_FONT).ok()?;
        let before = db.len();
        db.load_font_data(data);
        let face = db.faces().nth(before)?;
        face.families.first().map(|(name, _)| name.clone())
    })
}

/// 含文本的 SVG 需要字体才能正确渲染；读取字体开销大，进程内只做一次
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            if let Some(family) = load_first(&mut db, SANS) {
                db.set_sans_serif_family(family);
            }
            if let Some(family) = load_first(&mut db, SERIF) {
                db.set_serif_family(family);
            }
            if let Some(family) = load_first(&mut db, MONOSPACE) {
                db.set_monospace_family(family);
            }
            load_first(&mut db, CJK);
            Arc::new(db)
        })
        .clone()
}

/// 解析 SVG；失败的常见原因是文件不是 SVG 或包含不支持的特性
pub fn parse(bytes: &[u8]) -> PluginResult<usvg::Tree> {
    let has_text = bytes.windows(5).any(|w| w == b"<text");
    let options = usvg::Options {
        fontdb: if has_text {
            fonts()
        } else {
            Arc::new(usvg::fontdb::Database::new())
        },
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
