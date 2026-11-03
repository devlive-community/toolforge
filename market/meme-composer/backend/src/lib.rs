//! 表情包工坊后端：把用户图片与文字层合成 SVG（图片 data URI 内嵌、
//! 文字系统字体渲染），首版模板：上图下字与上下字幕。文字换行与
//! 缩字上限在 Rust 计算，前端负责导出 PNG。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};
use tf_plugin_guest::net;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct MemeComposer {
    manifest: Manifest,
}

impl Default for MemeComposer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for MemeComposer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "compose" => to_value(compose(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(MemeComposer);

const MAX_IMAGE: u64 = 8 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 用户选择的图片路径
    pub image_path: String,
    /// 上方文字
    #[serde(default)]
    pub top_text: Option<String>,
    /// 下方文字
    #[serde(default)]
    pub bottom_text: Option<String>,
    /// 画布宽（像素，200–2000）
    #[serde(default)]
    pub width: Option<u32>,
    /// 字色与描边
    #[serde(default)]
    pub text_color: Option<String>,
    #[serde(default)]
    pub stroke_color: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub svg: String,
    pub width: u32,
    pub height: u32,
    /// 每行字符数（换行结果，供前端核对）
    pub wrapped: usize,
}

fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        // 全角按 2、半角按 1 计宽
        let char_width = if ch.is_ascii() { 1 } else { 2 };
        if width + char_width > max_chars * 2 {
            lines.push(std::mem::take(&mut current));
            width = 0;
        }
        current.push(ch);
        width += char_width;
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn compose(args: Args) -> PluginResult<Output> {
    let width = args.width.unwrap_or(600).clamp(200, 2000);
    let top = args.top_text.unwrap_or_default();
    let bottom = args.bottom_text.unwrap_or_default();
    if top.chars().count() > MAX_TEXT_CHARS || bottom.chars().count() > MAX_TEXT_CHARS {
        return Err(PluginError::new("meme.text_too_long").with("max", MAX_TEXT_CHARS));
    }
    let image_size = net::file_size(&args.image_path)
        .map_err(|e| PluginError::new("meme.read_failed").with("detail", e.code))?;
    if image_size > MAX_IMAGE {
        return Err(PluginError::new("meme.image_too_large").with("max", MAX_IMAGE));
    }
    let bytes = net::read_to_end(&args.image_path, MAX_IMAGE)
        .map_err(|e| PluginError::new("meme.read_failed").with("detail", e.code))?;
    // 用 image crate 验证并取尺寸，同时按画布宽等比取景高度
    let image = image::load_from_memory(&bytes)
        .map_err(|e| PluginError::new("meme.bad_image").with("detail", e.to_string()))?;
    let (image_width, image_height) = (image.width(), image.height());
    let image_height_on_canvas =
        ((image_height as f64 / image_width as f64) * width as f64).min(width as f64 * 0.8) as u32;
    let text_color = args.text_color.unwrap_or_else(|| "#ffffff".into());
    let stroke_color = args.stroke_color.unwrap_or_else(|| "#000000".into());
    validate_color(&text_color)?;
    validate_color(&stroke_color)?;
    let data_uri = format!(
        "data:image/{};base64,{}",
        image_type(&bytes),
        base64_encode(&bytes)
    );
    let height = image_height_on_canvas + 160;
    let font_size = width as f64 / 14.0;
    let max_chars = (width as f64 / font_size * 1.1).floor() as usize;
    let top_lines = if top.is_empty() {
        Vec::new()
    } else {
        wrap(&top, max_chars)
    };
    let bottom_lines = if bottom.is_empty() {
        Vec::new()
    } else {
        wrap(&bottom, max_chars)
    };
    let mut svg = String::with_capacity(data_uri.len() / 4 + 1024);
    svg.push_str(&format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>"#
    ));
    svg.push_str(&format!(
        r#"<image x='0' y='0' width='{width}' height='{image_height_on_canvas}' href='{data_uri}' preserveAspectRatio='xMidYMid slice'/>"#
    ));
    for (index, line) in top_lines.iter().enumerate() {
        let y = 10.0 + font_size * (index as f64 + 0.9);
        emit_text(
            &mut svg,
            line,
            width as f64 / 2.0,
            y,
            font_size,
            &text_color,
            &stroke_color,
        );
    }
    let bottom_base = image_height_on_canvas as f64 + 20.0;
    for (index, line) in bottom_lines.iter().enumerate() {
        let y = bottom_base + font_size * (index as f64 + 0.9);
        if y > height as f64 - 6.0 {
            return Err(PluginError::new("meme.text_overflow"));
        }
        emit_text(
            &mut svg,
            line,
            width as f64 / 2.0,
            y,
            font_size,
            &text_color,
            &stroke_color,
        );
    }
    svg.push_str("</svg>");
    Ok(Output {
        svg,
        width,
        height,
        wrapped: top_lines.len() + bottom_lines.len(),
    })
}

fn emit_text(svg: &mut String, line: &str, x: f64, y: f64, size: f64, fill: &str, stroke: &str) {
    svg.push_str(&format!(
        r#"<text x='{x:.1}' y='{y:.1}' text-anchor='middle' font-family='sans-serif' font-weight='bold' font-size='{size:.1}' fill='{fill}' stroke='{stroke}' stroke-width='{:.1}' paint-order='stroke'>{line}</text>"#,
        size / 8.0
    ));
}

fn image_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8") {
        "jpeg"
    } else {
        "png"
    }
}

/// 标准base64（无依赖实现，输出不换行）
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) as usize & 63] as char);
        out.push(TABLE[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn validate_color(color: &str) -> PluginResult<()> {
    if color.starts_with('#')
        && color.len() <= 9
        && color[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(PluginError::new("meme.bad_color").with("color", color))
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
