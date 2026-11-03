//! 图片 → 字符画：按字符格取样、亮度调整、梯度映射与 ANSI 着色。

use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use tf_plugin_api::{PluginError, PluginResult};

//（亮度取样用 ImageBuffer 的固有方法，无需 trait 导入）
/// 输入图片像素上限（与 low-poly 一致）
pub const MAX_INPUT_PIXELS: u64 = 24_000_000;
/// 每行字符数范围
pub const MIN_COLS: u32 = 8;
pub const MAX_COLS: u32 = 500;
/// 输出行数上限
pub const MAX_ROWS: u32 = 500;
/// 默认字符格宽高比：等宽字体字符高约为宽的 2 倍
pub const DEFAULT_ASPECT: f64 = 0.5;
/// 透明判定阈值
const ALPHA_THRESHOLD: u8 = 128;

#[derive(Debug, Clone)]
pub struct Options {
    pub cols: u32,
    /// 由浅到深的符号梯度（已校验，至少 2 个字符）
    pub chars: String,
    pub brightness: f64,
    pub contrast: f64,
    pub invert: bool,
    pub color: bool,
    pub aspect: f64,
    /// true：透明像素留空；false：与底色合成
    pub keep_alpha: bool,
    pub bg: Rgba<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Output {
    /// 无任何控制字符（除换行）的纯文本
    pub plain: String,
    /// 彩色版：SGR 24 位前景色，每行结束重置；color=false 时与 plain 相同
    pub ansi: String,
    pub cols: u32,
    pub rows: u32,
}

pub fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 按字符格宽高比从像素尺寸换算行数，并钳制到上限
pub fn rows_for(width: u32, height: u32, cols: u32, aspect: f64) -> u32 {
    let aspect = aspect.clamp(0.3, 1.0);
    let raw = cols as f64 * (height as f64 / width as f64) * aspect;
    (raw.round().max(1.0) as u32).min(MAX_ROWS)
}

/// 把图片转换为字符画。梯度约定「由浅到深」，亮像素取深符号。
pub fn convert(img: &RgbaImage, opts: &Options) -> PluginResult<Output> {
    let cols = opts.cols.clamp(MIN_COLS, MAX_COLS);
    if opts.cols < MIN_COLS || opts.cols > MAX_COLS {
        return Err(errp("ascii.bad_width", "width", opts.cols as i64));
    }
    let gradient: Vec<char> = opts.chars.chars().collect();
    if gradient.len() < 2 {
        return Err(errp(
            "ascii.bad_custom",
            "reason",
            "符号梯度至少需要两个字符 / gradient needs at least 2 characters",
        ));
    }
    let (iw, ih) = img.dimensions();
    if iw == 0 || ih == 0 {
        return Err(errp("ascii.decode_failed", "detail", "empty image"));
    }
    let rows = rows_for(iw, ih, cols, opts.aspect);

    // 按字符格缩放：CatmullRom 高质量重采样，每格颜色即该格的平均取样
    let sample = image::imageops::resize(img, cols, rows, FilterType::CatmullRom);

    let mut plain = String::with_capacity(((cols + 1) * rows) as usize);
    let mut ansi = String::with_capacity(((cols + 1) * rows) as usize * 4);
    for y in 0..rows {
        for x in 0..cols {
            let px = sample.get_pixel(x, y);
            if opts.keep_alpha && px.0[3] < ALPHA_THRESHOLD {
                // 透明留空：纯文本与彩色版都保持空格
                plain.push(' ');
                ansi.push(' ');
                continue;
            }
            let blended = blend_over(px, &opts.bg);
            let luma = luma01(&blended);
            let adjusted = adjust(luma, opts.brightness, opts.contrast);
            let glyph = gradient[char_index(adjusted, gradient.len(), opts.invert)];
            plain.push(glyph);
            if opts.color {
                let [r, g, b, _] = blended.0;
                ansi.push_str(&format!("\u{1b}[38;2;{r};{g};{b}m{glyph}"));
            } else {
                ansi.push(glyph);
            }
        }
        if opts.color {
            // 每行结束重置颜色，颜色不会带到作品之后
            ansi.push_str("\u{1b}[0m");
        }
        plain.push('\n');
        ansi.push('\n');
    }
    Ok(Output {
        plain,
        ansi,
        cols,
        rows,
    })
}

/// 像素按 alpha 合成到底色上；不透明像素原样返回
fn blend_over(px: &Rgba<u8>, bg: &Rgba<u8>) -> Rgba<u8> {
    let a = px.0[3] as f64 / 255.0;
    if a >= 1.0 {
        return *px;
    }
    let mix = |fg: u8, bgc: u8| -> u8 { (fg as f64 * a + bgc as f64 * (1.0 - a)).round() as u8 };
    Rgba([
        mix(px.0[0], bg.0[0]),
        mix(px.0[1], bg.0[1]),
        mix(px.0[2], bg.0[2]),
        255,
    ])
}

/// ITU-R BT.709 亮度，归一到 0-1
fn luma01(px: &Rgba<u8>) -> f64 {
    let [r, g, b, _] = px.0;
    (0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)) / 255.0
}

/// 亮度调整：以 0.5 为中点做对比度缩放，再叠加亮度偏移
fn adjust(luma: f64, brightness: f64, contrast: f64) -> f64 {
    let v = (luma - 0.5) * contrast + 0.5 + brightness;
    v.clamp(0.0, 1.0)
}

/// 亮度 → 梯度索引：亮取深（梯度末尾），反相取反
fn char_index(luma: f64, len: usize, invert: bool) -> usize {
    let mut idx = (luma * (len - 1) as f64).round() as usize;
    idx = idx.min(len - 1);
    if invert { len - 1 - idx } else { idx }
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
