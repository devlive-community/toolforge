//! ANSI SGR 解析：把颜色 / 粗体等转义序列转换为样式区间，其余控制序列直接丢弃。

use serde::Serialize;

/// 颜色：0–15 为终端基本色（由界面主题决定具体颜色），其余为 #rrggbb
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Color {
    Basic(u8),
    Rgb(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<Color>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dim: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub inverse: bool,
}

impl Style {
    pub fn is_plain(&self) -> bool {
        *self == Style::default()
    }
}

/// xterm 256 色中 16–255 的 RGB 值
fn palette(index: u8) -> Color {
    if index < 16 {
        return Color::Basic(index);
    }
    let (r, g, b) = if index >= 232 {
        let level = 8 + (index - 232) * 10;
        (level, level, level)
    } else {
        let n = index - 16;
        let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        (step(n / 36), step((n / 6) % 6), step(n % 6))
    };
    Color::Rgb(format!("#{r:02x}{g:02x}{b:02x}"))
}

/// 解析扩展颜色参数（38;5;n 或 38;2;r;g;b），返回颜色与消耗的参数个数
fn extended(params: &[u16]) -> (Option<Color>, usize) {
    match params.first() {
        Some(5) => (params.get(1).map(|n| palette((*n).min(255) as u8)), 2),
        Some(2) => match (params.get(1), params.get(2), params.get(3)) {
            (Some(r), Some(g), Some(b)) => (
                Some(Color::Rgb(format!(
                    "#{:02x}{:02x}{:02x}",
                    (*r).min(255),
                    (*g).min(255),
                    (*b).min(255)
                ))),
                4,
            ),
            _ => (None, params.len()),
        },
        _ => (None, 1),
    }
}

fn apply(style: &mut Style, params: &[u16]) {
    if params.is_empty() {
        *style = Style::default();
        return;
    }
    let mut i = 0;
    while i < params.len() {
        let code = params[i];
        i += 1;
        match code {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            7 => style.inverse = true,
            9 => style.strike = true,
            21 | 22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            27 => style.inverse = false,
            29 => style.strike = false,
            30..=37 => style.fg = Some(Color::Basic((code - 30) as u8)),
            39 => style.fg = None,
            40..=47 => style.bg = Some(Color::Basic((code - 40) as u8)),
            49 => style.bg = None,
            90..=97 => style.fg = Some(Color::Basic((code - 90 + 8) as u8)),
            100..=107 => style.bg = Some(Color::Basic((code - 100 + 8) as u8)),
            38 | 48 => {
                let (color, used) = extended(&params[i..]);
                i += used;
                if let Some(color) = color {
                    if code == 38 {
                        style.fg = Some(color);
                    } else {
                        style.bg = Some(color);
                    }
                }
            }
            _ => {}
        }
    }
}

/// 一段文本（原始字节中的范围）与它的样式
#[derive(Debug, PartialEq)]
pub struct Run {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

/// 把一行原始字节切分为若干文本段；转义序列本身不包含在任何段中
pub fn runs(bytes: &[u8]) -> Vec<Run> {
    let mut out = Vec::new();
    let mut style = Style::default();
    let mut i = 0;
    let mut text_start = 0;
    let flush = |out: &mut Vec<Run>, start: usize, end: usize, style: &Style| {
        if end > start {
            out.push(Run {
                start,
                end,
                style: style.clone(),
            });
        }
    };
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            i += 1;
            continue;
        }
        flush(&mut out, text_start, i, &style);
        match bytes.get(i + 1) {
            Some(b'[') => {
                let params_start = i + 2;
                let mut j = params_start;
                while j < bytes.len() && !(0x40..=0x7e).contains(&bytes[j]) {
                    j += 1;
                }
                if bytes.get(j) == Some(&b'm') {
                    let params: Vec<u16> = std::str::from_utf8(&bytes[params_start..j])
                        .unwrap_or("")
                        .split([';', ':'])
                        .map(|p| p.parse().unwrap_or(0))
                        .collect();
                    let params = if bytes[params_start..j].is_empty() {
                        Vec::new()
                    } else {
                        params
                    };
                    apply(&mut style, &params);
                }
                i = (j + 1).min(bytes.len());
            }
            Some(b']') => {
                let mut j = i + 2;
                while j < bytes.len()
                    && bytes[j] != 0x07
                    && !(bytes[j] == 0x1b && bytes.get(j + 1) == Some(&b'\\'))
                {
                    j += 1;
                }
                i = if bytes.get(j) == Some(&0x07) {
                    j + 1
                } else {
                    (j + 2).min(bytes.len())
                };
            }
            // ESC 中间字节（0x20–0x2f，如字符集切换 ESC ( B）… 结束字节
            Some(_) => {
                let mut j = i + 1;
                while j < bytes.len() && (0x20..=0x2f).contains(&bytes[j]) {
                    j += 1;
                }
                i = (j + 1).min(bytes.len());
            }
            None => i += 1,
        }
        text_start = i;
    }
    flush(&mut out, text_start, bytes.len(), &style);
    out
}

#[cfg(test)]
#[path = "ansi_test.rs"]
mod tests;
