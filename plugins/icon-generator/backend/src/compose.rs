//! 合成图标：留白、背景色、圆角或圆形遮罩，再缩放到各个尺寸。

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

/// 合成用的母版边长
pub const MASTER: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    /// 四周留白，占边长的比例（0–0.4）
    #[serde(default)]
    pub padding: f32,
    /// 背景色 #rrggbb；为空表示透明
    #[serde(default, deserialize_with = "color")]
    pub background: Option<[u8; 3]>,
    /// 圆角半径，占边长的比例（0–0.5）
    #[serde(default)]
    pub radius: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            padding: 0.0,
            background: None,
            radius: 0.0,
        }
    }
}

pub fn parse_color(text: &str) -> Option<[u8; 3]> {
    let hex = text.trim().trim_start_matches('#');
    let hex = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_owned(),
        _ => return None,
    };
    let value = u32::from_str_radix(&hex, 16).ok()?;
    Some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

fn color<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Option<[u8; 3]>, D::Error> {
    let text: Option<String> = Option::deserialize(de)?;
    match text.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(text) => parse_color(text)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("invalid color")),
    }
}

impl Style {
    pub fn validate(&self) -> PluginResult<()> {
        if !(0.0..=0.4).contains(&self.padding) || !(0.0..=0.5).contains(&self.radius) {
            return Err(PluginError::new("icon.invalid_style"));
        }
        Ok(())
    }
}

/// 图标的外形
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// 用户设置的圆角
    Rounded,
    /// 必须不透明且不带圆角（iOS 由系统裁切）
    Opaque,
    /// 圆形（Android 圆形图标）
    Circle,
    /// macOS 模板：1024 中 824 的圆角方块，四周留出阴影区域
    Mac,
}

/// 圆角矩形内的覆盖率（抗锯齿），坐标为像素中心
fn coverage(x: f32, y: f32, left: f32, top: f32, size: f32, radius: f32) -> f32 {
    let half = size / 2.0;
    let (cx, cy) = (left + half, top + half);
    let r = radius.min(half);
    let qx = (x - cx).abs() - (half - r);
    let qy = (y - cy).abs() - (half - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let distance = outside + qx.max(qy).min(0.0) - r;
    (0.5 - distance).clamp(0.0, 1.0)
}

fn over(dst: &mut Rgba<u8>, src: Rgba<u8>, mask: f32) {
    let sa = src[3] as f32 / 255.0 * mask;
    if sa <= 0.0 {
        return;
    }
    let da = dst[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    for i in 0..3 {
        let c = (src[i] as f32 * sa + dst[i] as f32 * da * (1.0 - sa)) / oa;
        dst[i] = c.round() as u8;
    }
    dst[3] = (oa * 255.0).round() as u8;
}

/// 合成边长为 `size` 的母版
pub fn master(source: &RgbaImage, style: &Style, shape: Shape, size: u32) -> RgbaImage {
    let s = size as f32;
    // 方块区域（macOS 模板留出阴影区域）
    let (tile_left, tile_size) = match shape {
        Shape::Mac => (s * 100.0 / 1024.0, s * 824.0 / 1024.0),
        _ => (0.0, s),
    };
    let radius = match shape {
        Shape::Rounded => style.radius * s,
        Shape::Opaque => 0.0,
        Shape::Circle => s / 2.0,
        Shape::Mac => tile_size * 0.2237,
    };
    let background = match shape {
        // 不允许透明的外形默认白色背景
        Shape::Opaque | Shape::Circle | Shape::Mac => Some(style.background.unwrap_or([255; 3])),
        Shape::Rounded => style.background,
    };
    // 圆形中的内容要留够边距，四角才不会被裁掉
    let padding = match shape {
        Shape::Circle => style.padding.max(0.15),
        _ => style.padding,
    };
    let inner = (tile_size * (1.0 - 2.0 * padding)).max(1.0);
    let (w, h) = source.dimensions();
    let scale = inner / w.max(h) as f32;
    let (fw, fh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    let fitted = imageops::resize(source, fw, fh, FilterType::Lanczos3);
    let ox = (tile_left + (tile_size - fw as f32) / 2.0).round() as i64;
    let oy = (tile_left + (tile_size - fh as f32) / 2.0).round() as i64;

    let mut out = RgbaImage::new(size, size);
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let mask = coverage(
            x as f32 + 0.5,
            y as f32 + 0.5,
            tile_left,
            tile_left,
            tile_size,
            radius,
        );
        if mask <= 0.0 {
            continue;
        }
        if let Some([r, g, b]) = background {
            over(pixel, Rgba([r, g, b, 255]), mask);
        }
        let (fx, fy) = (x as i64 - ox, y as i64 - oy);
        if fx >= 0 && fy >= 0 && (fx as u32) < fw && (fy as u32) < fh {
            over(pixel, *fitted.get_pixel(fx as u32, fy as u32), mask);
        }
    }
    if shape == Shape::Opaque {
        for pixel in out.pixels_mut() {
            pixel[3] = 255;
        }
    }
    out
}

pub fn scaled(master: &RgbaImage, size: u32) -> RgbaImage {
    if master.width() == size {
        return master.clone();
    }
    imageops::resize(master, size, size, FilterType::Lanczos3)
}

#[cfg(test)]
#[path = "compose_test.rs"]
mod tests;
