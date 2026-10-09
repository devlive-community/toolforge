//! 编码与绘制：rxing 生成条空或模块矩阵，自己画成 SVG（EAN / UPC 带护线与分组数字），PNG 由 resvg 栅格化。

use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};
use rxing::common::BitMatrix;
use rxing::oned::{
    CodaBarWriter, Code39Writer, Code93Writer, Code128Writer, EAN8Writer, EAN13Writer, ITFWriter,
    OneDimensionalCodeWriter,
};
use rxing::{BarcodeFormat, EncodeHints, Writer};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::formats::Format;

pub const MAX_SCALE: u32 = 20;
pub const MAX_HEIGHT: u32 = 300;
pub const MAX_MARGIN: u32 = 50;
/// PNG 最大边长（像素）
const MAX_PIXELS: u32 = 16_000;
/// 一维码上下留白（模块）
const PAD_Y: u32 = 4;
const FONT: &str =
    "ui-monospace, Menlo, Consolas, 'DejaVu Sans Mono', 'Liberation Mono', monospace";

pub enum Symbol {
    Linear(Vec<bool>),
    Matrix {
        width: u32,
        height: u32,
        bits: Vec<bool>,
    },
}

fn encode_failed(e: rxing::Exceptions) -> PluginError {
    PluginError::new("barcode.encode_failed").with("detail", e.to_string())
}

fn matrix(bits: BitMatrix) -> Symbol {
    let (width, height) = (bits.getWidth(), bits.getHeight());
    let cells = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| bits.get(x, y))
        .collect();
    Symbol::Matrix {
        width,
        height,
        bits: cells,
    }
}

/// 把 UTF-8 字节逐个当作 ISO-8859-1 字符
fn latin1_bytes(text: &str) -> String {
    text.bytes().map(char::from).collect()
}

pub fn encode(format: Format, content: &str) -> PluginResult<Symbol> {
    let linear =
        |bars: rxing::common::Result<Vec<bool>>| bars.map(Symbol::Linear).map_err(encode_failed);
    let two_d = |writer: &dyn Writer, format: BarcodeFormat| {
        let hints = EncodeHints {
            Margin: Some("0".into()),
            CharacterSet: (!content.is_ascii() && format != BarcodeFormat::DATA_MATRIX)
                .then(|| "UTF-8".into()),
            ..Default::default()
        };
        // rxing 的 Data Matrix ECI 读写不可靠：直接按字节写入 UTF-8（多数扫码软件会自动识别）
        let bytes;
        let content = if format == BarcodeFormat::DATA_MATRIX {
            bytes = latin1_bytes(content);
            bytes.as_str()
        } else {
            content
        };
        writer
            .encode_with_hints(content, &format, 0, 0, &hints)
            .map(matrix)
            .map_err(encode_failed)
    };
    match format {
        Format::Ean13 => linear(EAN13Writer.encode_oned(content)),
        Format::Ean8 => linear(EAN8Writer.encode_oned(content)),
        // UPC-A 就是首位为 0 的 EAN-13
        Format::UpcA => linear(EAN13Writer.encode_oned(&format!("0{content}"))),
        Format::Code128 => linear(Code128Writer.encode_oned(content)),
        Format::Code39 => linear(Code39Writer.encode_oned(content)),
        Format::Code93 => linear(Code93Writer.encode_oned(content)),
        Format::Itf => linear(ITFWriter.encode_oned(content)),
        Format::Codabar => linear(CodaBarWriter.encode_oned(content)),
        Format::DataMatrix => two_d(
            &rxing::datamatrix::DataMatrixWriter,
            BarcodeFormat::DATA_MATRIX,
        ),
        Format::Pdf417 => two_d(&rxing::pdf417::PDF417Writer, BarcodeFormat::PDF_417),
        Format::Aztec => two_d(&rxing::aztec::AztecWriter, BarcodeFormat::AZTEC),
    }
}

fn default_height() -> u32 {
    60
}

fn default_scale() -> u32 {
    3
}

fn yes() -> bool {
    true
}

fn default_dark() -> String {
    "#000000".into()
}

fn default_light() -> String {
    "#ffffff".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    /// 一维码条高（模块）
    #[serde(default = "default_height")]
    pub height: u32,
    /// 每个模块的像素数
    #[serde(default = "default_scale")]
    pub scale: u32,
    /// 静区（模块）；不填按码制默认
    #[serde(default)]
    pub margin: Option<u32>,
    #[serde(default = "yes")]
    pub show_text: bool,
    #[serde(default = "default_dark")]
    pub dark: String,
    #[serde(default = "default_light")]
    pub light: String,
}

impl Style {
    pub fn validate(&self) -> PluginResult<()> {
        if !(1..=MAX_SCALE).contains(&self.scale) {
            return Err(PluginError::new("barcode.invalid_scale").with("max", MAX_SCALE));
        }
        if !(10..=MAX_HEIGHT).contains(&self.height) {
            return Err(PluginError::new("barcode.invalid_height")
                .with("min", 10)
                .with("max", MAX_HEIGHT));
        }
        if self.margin.is_some_and(|m| m > MAX_MARGIN) {
            return Err(PluginError::new("barcode.invalid_margin").with("max", MAX_MARGIN));
        }
        Ok(())
    }
}

/// 解析 #rgb / #rrggbb / #rrggbbaa，返回 SVG 可用的颜色
pub fn parse_color(value: &str, field: &str) -> PluginResult<String> {
    let hex = value.trim().trim_start_matches('#');
    let valid = matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err(PluginError::new("barcode.invalid_color")
            .with("field", field)
            .with("value", value));
    }
    Ok(format!("#{}", hex.to_ascii_lowercase()))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// EAN / UPC 的护线（向下延长的条）与数字位置（以条码起点为 0 的模块坐标）
struct Retail {
    guards: &'static [(usize, usize)],
    /// (字符下标, 中心 x)
    digits: Vec<(usize, f32)>,
}

fn retail(format: Format) -> Option<Retail> {
    let cells = |first: usize, start: usize, count: usize| {
        (0..count).map(move |i| (first + i, (start + 7 * i) as f32 + 3.5))
    };
    match format {
        Format::Ean13 => Some(Retail {
            guards: &[(0, 3), (45, 50), (92, 95)],
            digits: std::iter::once((0, -5.0))
                .chain(cells(1, 3, 6))
                .chain(cells(7, 50, 6))
                .collect(),
        }),
        Format::Ean8 => Some(Retail {
            guards: &[(0, 3), (31, 36), (64, 67)],
            digits: cells(0, 3, 4).chain(cells(4, 36, 4)).collect(),
        }),
        Format::UpcA => Some(Retail {
            guards: &[(0, 10), (45, 50), (85, 95)],
            digits: std::iter::once((0, -5.0))
                .chain(cells(1, 10, 5))
                .chain(cells(6, 50, 5))
                .chain(std::iter::once((11, 100.0)))
                .collect(),
        }),
        _ => None,
    }
}

#[derive(Debug)]
pub struct Drawing {
    pub svg: String,
    /// 按 scale 换算后的像素尺寸
    pub pixels: (u32, u32),
}

fn bar_path(bars: &[bool], x0: u32, y: u32, height: impl Fn(usize) -> u32) -> String {
    let mut path = String::new();
    let mut x = 0;
    while x < bars.len() {
        if !bars[x] {
            x += 1;
            continue;
        }
        let start = x;
        let h = height(x);
        while x < bars.len() && bars[x] && height(x) == h {
            x += 1;
        }
        path.push_str(&format!(
            "M{} {y}h{}v{h}h-{}z",
            x0 as usize + start,
            x - start,
            x - start
        ));
    }
    path
}

pub fn draw(format: Format, symbol: &Symbol, text: &str, style: &Style) -> PluginResult<Drawing> {
    style.validate()?;
    let dark = parse_color(&style.dark, "dark")?;
    let light = parse_color(&style.light, "light")?;
    let mut body = String::new();
    let (width, height) = match symbol {
        Symbol::Linear(bars) => {
            let layout = style.show_text.then(|| retail(format)).flatten();
            // 首尾数字写在静区里，静区至少 9 个模块
            let margin = style
                .margin
                .unwrap_or(10)
                .max(if layout.is_some() { 9 } else { 0 });
            let bar_h = style.height;
            let guard_h = bar_h + 5;
            let font = 8.0_f32;
            let in_guard = |x: usize| {
                layout
                    .as_ref()
                    .is_some_and(|l| l.guards.iter().any(|(a, b)| (*a..*b).contains(&x)))
            };
            let path = bar_path(bars, margin, PAD_Y, |x| {
                if in_guard(x) { guard_h } else { bar_h }
            });
            body.push_str(&format!(r#"<path d="{path}" fill="{dark}"/>"#));
            let width = bars.len() as u32 + margin * 2;
            let mut height = PAD_Y * 2 + bar_h;
            if style.show_text {
                let chars: Vec<char> = text.chars().collect();
                match &layout {
                    Some(l) => {
                        // 数字顶端紧贴普通条下沿，护线延伸到数字中部
                        height += 9;
                        let y = (PAD_Y + bar_h) as f32 + 1.0 + font * 0.75;
                        for (i, cx) in &l.digits {
                            if let Some(c) = chars.get(*i) {
                                body.push_str(&format!(
                                    r#"<text x="{}" y="{y}" font-size="{font}" text-anchor="middle" font-family="{FONT}" fill="{dark}">{c}</text>"#,
                                    margin as f32 + cx
                                ));
                            }
                        }
                    }
                    None => {
                        // 文字太长时缩小字号，不超过条码宽度
                        let size =
                            (bars.len() as f32 / (chars.len().max(1) as f32 * 0.62)).min(10.0);
                        height += (size + 3.0).ceil() as u32;
                        body.push_str(&format!(
                            r#"<text x="{}" y="{}" font-size="{size:.2}" text-anchor="middle" font-family="{FONT}" fill="{dark}">{}</text>"#,
                            width as f32 / 2.0,
                            (PAD_Y + bar_h) as f32 + 1.5 + size * 0.85,
                            escape(text)
                        ));
                    }
                }
            }
            (width, height)
        }
        Symbol::Matrix {
            width,
            height,
            bits,
        } => {
            let margin = style.margin.unwrap_or(2);
            let mut path = String::new();
            for (row, line) in bits.chunks(*width as usize).enumerate() {
                path.push_str(&bar_path(line, margin, margin + row as u32, |_| 1));
            }
            body.push_str(&format!(r#"<path d="{path}" fill="{dark}"/>"#));
            (width + margin * 2, height + margin * 2)
        }
    };
    let pixels = (width * style.scale, height * style.scale);
    if pixels.0.max(pixels.1) > MAX_PIXELS {
        return Err(PluginError::new("barcode.too_large").with("max", MAX_PIXELS));
    }
    let svg = format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{pw}" height="{ph}" viewBox="0 0 {w} {h}" shape-rendering="crispEdges">"#,
            r#"<rect width="{w}" height="{h}" fill="{light}"/>{body}</svg>"#,
            "\n"
        ),
        pw = pixels.0,
        ph = pixels.1,
        w = width,
        h = height,
        light = light,
        body = body,
    );
    Ok(Drawing { svg, pixels })
}

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

pub fn png(drawing: &Drawing) -> PluginResult<Vec<u8>> {
    let failed = |detail: String| PluginError::new("barcode.render_failed").with("detail", detail);
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&drawing.svg, &options).map_err(|e| failed(e.to_string()))?;
    let (w, h) = drawing.pixels;
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or_else(|| failed("empty".into()))?;
    // 根元素的 width / height 已是像素尺寸，viewBox 负责把模块映射过去
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| failed(e.to_string()))
}

#[cfg(test)]
#[path = "render_test.rs"]
mod tests;
