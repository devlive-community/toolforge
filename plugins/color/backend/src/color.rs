use csscolorparser::Color;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

const STEPS: [f32; 9] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];

#[derive(Deserialize)]
pub struct Args {
    input: String,
    /// 可选的对比色（前景 / 背景对比度）
    #[serde(default)]
    compare: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Format {
    pub key: &'static str,
    pub value: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contrast {
    pub ratio: f64,
    /// WCAG 2.x：普通文本 AA ≥ 4.5，AAA ≥ 7；大号文本 AA ≥ 3，AAA ≥ 4.5
    pub aa: bool,
    pub aaa: bool,
    pub aa_large: bool,
    pub aaa_large: bool,
}

#[derive(Debug, Serialize)]
pub struct Harmony {
    pub key: &'static str,
    pub colors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// 不含透明度的 #rrggbb，用于色块展示
    pub hex: String,
    pub alpha: f64,
    pub name: Option<&'static str>,
    pub formats: Vec<Format>,
    pub luminance: f64,
    /// 深色（与白色对比更好）
    pub dark: bool,
    /// 放在该颜色上可读性更好的文字颜色（黑或白）
    pub text: &'static str,
    pub on_white: Contrast,
    pub on_black: Contrast,
    pub compare: Option<Contrast>,
    pub compare_hex: Option<String>,
    pub tints: Vec<String>,
    pub shades: Vec<String>,
    pub harmonies: Vec<Harmony>,
}

fn round(value: f32, digits: i32) -> f64 {
    let factor = 10f64.powi(digits);
    (value as f64 * factor).round() / factor
}

pub fn parse(input: &str) -> PluginResult<Color> {
    let input = input.trim();
    if input.is_empty() {
        return Err(PluginError::new("color.empty"));
    }
    // 兼容 0xRRGGBB / 0xAARRGGBB（Android 等平台的 ARGB 写法）
    if let Some(hex) = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
    {
        let value = u32::from_str_radix(hex, 16)
            .map_err(|_| PluginError::new("color.invalid").with("input", input))?;
        let [a, r, g, b] = value.to_be_bytes();
        let a = if hex.len() > 6 { a } else { 255 };
        return Ok(Color::from_rgba8(r, g, b, a));
    }
    Color::from_html(input).map_err(|_| PluginError::new("color.invalid").with("input", input))
}

fn opaque_hex(color: &Color) -> String {
    let [r, g, b, _] = color.to_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// WCAG 相对亮度
pub fn luminance(color: &Color) -> f64 {
    let channel = |c: f32| {
        let c = c.clamp(0.0, 1.0) as f64;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

pub fn contrast(a: &Color, b: &Color) -> Contrast {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    let ratio = ((hi + 0.05) / (lo + 0.05) * 100.0).round() / 100.0;
    Contrast {
        ratio,
        aa: ratio >= 4.5,
        aaa: ratio >= 7.0,
        aa_large: ratio >= 3.0,
        aaa_large: ratio >= 4.5,
    }
}

fn cmyk(color: &Color) -> String {
    let (r, g, b) = (
        color.r.clamp(0.0, 1.0),
        color.g.clamp(0.0, 1.0),
        color.b.clamp(0.0, 1.0),
    );
    let k = 1.0 - r.max(g).max(b);
    let part = |c: f32| {
        if k >= 1.0 {
            0.0
        } else {
            (1.0 - c - k) / (1.0 - k)
        }
    };
    let pct = |v: f32| (v * 100.0).round();
    format!(
        "cmyk({}% {}% {}% {}%)",
        pct(part(r)),
        pct(part(g)),
        pct(part(b)),
        pct(k)
    )
}

fn formats(color: &Color) -> Vec<Format> {
    let [r, g, b, a] = color.to_rgba8();
    let alpha = round(color.a, 3);
    let [h, s, v, _] = color.to_hsva();
    let mut list = vec![
        Format {
            key: "hex",
            value: color.to_css_hex(),
        },
        Format {
            key: "rgb",
            value: color.to_css_rgb(),
        },
        Format {
            key: "rgbLegacy",
            value: if color.a < 1.0 {
                format!("rgba({r}, {g}, {b}, {alpha})")
            } else {
                format!("rgb({r}, {g}, {b})")
            },
        },
        Format {
            key: "hsl",
            value: color.to_css_hsl(),
        },
        Format {
            key: "hsv",
            value: format!(
                "hsv({} {}% {}%)",
                round(if h.is_nan() { 0.0 } else { h }, 1),
                (s * 100.0).round(),
                (v * 100.0).round()
            ),
        },
        Format {
            key: "hwb",
            value: color.to_css_hwb(),
        },
        Format {
            key: "cmyk",
            value: cmyk(color),
        },
        Format {
            key: "lab",
            value: color.to_css_lab(),
        },
        Format {
            key: "lch",
            value: color.to_css_lch(),
        },
        Format {
            key: "oklab",
            value: color.to_css_oklab(),
        },
        Format {
            key: "oklch",
            value: color.to_css_oklch(),
        },
        Format {
            key: "argb",
            value: format!("0x{a:02X}{r:02X}{g:02X}{b:02X}"),
        },
        Format {
            key: "vec",
            value: format!(
                "{}, {}, {}, {}",
                round(color.r, 3),
                round(color.g, 3),
                round(color.b, 3),
                alpha
            ),
        },
    ];
    if let Some(name) = color.name() {
        list.insert(
            1,
            Format {
                key: "name",
                value: name.to_owned(),
            },
        );
    }
    list
}

/// 在 OKLCH 空间旋转色相，保持明度与彩度
fn rotate(color: &Color, degrees: f32) -> String {
    let [l, c, h, a] = color.to_oklcha();
    let rotated = Color::from_oklcha(l, c, h + degrees.to_radians(), a);
    opaque_hex(&clamp(rotated))
}

fn clamp(color: Color) -> Color {
    Color::new(
        color.r.clamp(0.0, 1.0),
        color.g.clamp(0.0, 1.0),
        color.b.clamp(0.0, 1.0),
        color.a,
    )
}

fn mix(color: &Color, with: &Color) -> Vec<String> {
    STEPS
        .iter()
        .map(|t| opaque_hex(&clamp(color.interpolate_oklab(with, *t))))
        .collect()
}

pub fn convert(args: Args) -> PluginResult<Output> {
    let color = parse(&args.input)?;
    let white = Color::new(1.0, 1.0, 1.0, 1.0);
    let black = Color::new(0.0, 0.0, 0.0, 1.0);
    let compare = match args.compare.trim() {
        "" => None,
        other => Some(parse(other).map_err(|e| e.with("field", "compare"))?),
    };
    let on_white = contrast(&color, &white);
    let on_black = contrast(&color, &black);

    Ok(Output {
        hex: opaque_hex(&color),
        alpha: round(color.a, 3),
        name: color.name(),
        formats: formats(&color),
        luminance: (luminance(&color) * 10_000.0).round() / 10_000.0,
        dark: on_white.ratio >= on_black.ratio,
        text: if on_white.ratio >= on_black.ratio {
            "#ffffff"
        } else {
            "#000000"
        },
        on_white,
        on_black,
        compare_hex: compare.as_ref().map(opaque_hex),
        compare: compare.as_ref().map(|other| contrast(&color, other)),
        tints: mix(&color, &white),
        shades: mix(&color, &black),
        harmonies: vec![
            Harmony {
                key: "complementary",
                colors: vec![opaque_hex(&color), rotate(&color, 180.0)],
            },
            Harmony {
                key: "analogous",
                colors: vec![
                    rotate(&color, -30.0),
                    opaque_hex(&color),
                    rotate(&color, 30.0),
                ],
            },
            Harmony {
                key: "triadic",
                colors: vec![
                    opaque_hex(&color),
                    rotate(&color, 120.0),
                    rotate(&color, 240.0),
                ],
            },
            Harmony {
                key: "tetradic",
                colors: vec![
                    opaque_hex(&color),
                    rotate(&color, 90.0),
                    rotate(&color, 180.0),
                    rotate(&color, 270.0),
                ],
            },
        ],
    })
}

#[cfg(test)]
#[path = "color_test.rs"]
mod tests;
