use std::io::Cursor;

use image::{ImageFormat, ImageReader, Rgba, RgbaImage};
use qrcode::{Color, EcLevel, QrCode};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

const MAX_SCALE: u32 = 64;
const MAX_MARGIN: u32 = 16;
const MAX_DECODE_FILE: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
pub enum Ecc {
    L,
    #[default]
    M,
    Q,
    H,
}

impl From<Ecc> for EcLevel {
    fn from(ecc: Ecc) -> Self {
        match ecc {
            Ecc::L => EcLevel::L,
            Ecc::M => EcLevel::M,
            Ecc::Q => EcLevel::Q,
            Ecc::H => EcLevel::H,
        }
    }
}

fn default_scale() -> u32 {
    8
}

fn default_margin() -> u32 {
    4
}

fn default_dark() -> String {
    "#000000".into()
}

fn default_light() -> String {
    "#ffffff".into()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub text: String,
    #[serde(default)]
    pub ecc: Ecc,
    /// 每个模块的像素数
    #[serde(default = "default_scale")]
    pub scale: u32,
    /// 静区宽度（模块数）
    #[serde(default = "default_margin")]
    pub margin: u32,
    #[serde(default = "default_dark")]
    pub dark: String,
    #[serde(default = "default_light")]
    pub light: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SaveFormat {
    Png,
    Svg,
}

#[derive(Deserialize)]
pub struct SaveArgs {
    #[serde(flatten)]
    options: Options,
    format: SaveFormat,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    /// PNG 预览（Data URI）
    pub image: String,
    pub version: i16,
    pub modules: usize,
    pub pixels: u32,
    pub bytes: usize,
}

#[derive(Deserialize)]
pub struct DecodeArgs {
    path: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Decoded {
    pub text: String,
    pub version: usize,
    pub ecc: Ecc,
}

/// rqrr 返回的是格式信息中的原始纠错位：0=M、1=L、2=H、3=Q
fn ecc_from_format_bits(bits: u16) -> Ecc {
    match bits {
        1 => Ecc::L,
        2 => Ecc::H,
        3 => Ecc::Q,
        _ => Ecc::M,
    }
}

/// 解析 #rgb / #rrggbb / #rrggbbaa
pub fn parse_hex(value: &str, field: &str) -> PluginResult<Rgba<u8>> {
    let invalid = || {
        PluginError::new("qr.invalid_color")
            .with("field", field)
            .with("value", value)
    };
    let hex = value.trim().trim_start_matches('#');
    let expanded: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => hex.to_owned(),
        _ => return Err(invalid()),
    };
    let byte = |i: usize| u8::from_str_radix(&expanded[i..i + 2], 16).map_err(|_| invalid());
    let alpha = if expanded.len() == 8 { byte(6)? } else { 255 };
    Ok(Rgba([byte(0)?, byte(2)?, byte(4)?, alpha]))
}

struct Matrix {
    code: QrCode,
    dark: Rgba<u8>,
    light: Rgba<u8>,
    scale: u32,
    margin: u32,
}

impl Matrix {
    fn new(options: &Options) -> PluginResult<Self> {
        if options.text.is_empty() {
            return Err(PluginError::new("qr.empty"));
        }
        if !(1..=MAX_SCALE).contains(&options.scale) {
            return Err(PluginError::new("qr.invalid_scale").with("max", MAX_SCALE));
        }
        if options.margin > MAX_MARGIN {
            return Err(PluginError::new("qr.invalid_margin").with("max", MAX_MARGIN));
        }
        let code = QrCode::with_error_correction_level(options.text.as_bytes(), options.ecc.into())
            .map_err(|_| PluginError::new("qr.too_long").with("bytes", options.text.len()))?;
        Ok(Self {
            code,
            dark: parse_hex(&options.dark, "dark")?,
            light: parse_hex(&options.light, "light")?,
            scale: options.scale,
            margin: options.margin,
        })
    }

    fn modules(&self) -> usize {
        self.code.width()
    }

    fn pixels(&self) -> u32 {
        (self.modules() as u32 + self.margin * 2) * self.scale
    }

    fn is_dark(&self, x: usize, y: usize) -> bool {
        self.code[(x, y)] == Color::Dark
    }

    fn png(&self) -> PluginResult<Vec<u8>> {
        let size = self.pixels();
        let (scale, margin) = (self.scale, self.margin);
        let image = RgbaImage::from_fn(size, size, |px, py| {
            let (x, y) = (px / scale, py / scale);
            let inside = x >= margin && y >= margin;
            let (mx, my) = (
                x.wrapping_sub(margin) as usize,
                y.wrapping_sub(margin) as usize,
            );
            if inside && mx < self.modules() && my < self.modules() && self.is_dark(mx, my) {
                self.dark
            } else {
                self.light
            }
        });
        let mut buffer = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .map_err(|e| PluginError::new("qr.encode_failed").with("detail", e.to_string()))?;
        Ok(buffer)
    }

    fn svg(&self) -> String {
        let css = |c: Rgba<u8>| {
            let [r, g, b, a] = c.0;
            if a == 255 {
                format!("#{r:02x}{g:02x}{b:02x}")
            } else {
                format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
            }
        };
        let total = self.modules() as u32 + self.margin * 2;
        let mut path = String::new();
        for y in 0..self.modules() {
            for x in 0..self.modules() {
                if self.is_dark(x, y) {
                    let (px, py) = (x as u32 + self.margin, y as u32 + self.margin);
                    path.push_str(&format!("M{px} {py}h1v1h-1z"));
                }
            }
        }
        let size = self.pixels();
        format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {total} {total}" shape-rendering="crispEdges">"#,
                r#"<rect width="{total}" height="{total}" fill="{light}"/><path d="{path}" fill="{dark}"/></svg>"#,
                "\n"
            ),
            size = size,
            total = total,
            light = css(self.light),
            dark = css(self.dark),
            path = path,
        )
    }
}

fn version_number(code: &QrCode) -> i16 {
    match code.version() {
        qrcode::Version::Normal(v) | qrcode::Version::Micro(v) => v,
    }
}

pub fn generate(options: Options) -> PluginResult<Generated> {
    let matrix = Matrix::new(&options)?;
    let png = matrix.png()?;
    Ok(Generated {
        image: format!(
            "data:image/png;base64,{}",
            data_encoding::BASE64.encode(&png)
        ),
        version: version_number(&matrix.code),
        modules: matrix.modules(),
        pixels: matrix.pixels(),
        bytes: options.text.len(),
    })
}

pub fn save(args: SaveArgs) -> PluginResult<()> {
    let matrix = Matrix::new(&args.options)?;
    let bytes = match args.format {
        SaveFormat::Png => matrix.png()?,
        SaveFormat::Svg => matrix.svg().into_bytes(),
    };
    std::fs::write(&args.path, bytes).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", args.path.as_str())
            .with("detail", e.to_string())
    })
}

/// 识别图片中的所有二维码
pub fn decode_bytes(bytes: &[u8]) -> PluginResult<Vec<Decoded>> {
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PluginError::new("qr.decode_failed").with("detail", e.to_string()))?
        .decode()
        .map_err(|e| PluginError::new("qr.decode_failed").with("detail", e.to_string()))?
        .to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
        image.width() as usize,
        image.height() as usize,
        |x, y| image.get_pixel(x as u32, y as u32).0[0],
    );
    let found: Vec<Decoded> = prepared
        .detect_grids()
        .into_iter()
        .filter_map(|grid| grid.decode().ok())
        .map(|(meta, text)| Decoded {
            text,
            version: meta.version.0,
            ecc: ecc_from_format_bits(meta.ecc_level),
        })
        .collect();
    if found.is_empty() {
        return Err(PluginError::new("qr.not_found"));
    }
    Ok(found)
}

pub fn decode(args: DecodeArgs) -> PluginResult<Vec<Decoded>> {
    let metadata = std::fs::metadata(&args.path)
        .map_err(|_| PluginError::new("fs.not_found").with("path", args.path.as_str()))?;
    if metadata.len() > MAX_DECODE_FILE {
        return Err(PluginError::new("qr.file_too_large").with("limit", "50 MB"));
    }
    let bytes = std::fs::read(&args.path).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", args.path.as_str())
            .with("detail", e.to_string())
    })?;
    decode_bytes(&bytes)
}

#[cfg(test)]
#[path = "qr_test.rs"]
mod tests;
