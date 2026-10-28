//! 笔画栅格化：画笔与橡皮。
//!
//! 一笔先在覆盖度缓冲里按「圆头印章」沿折线逐段铺开（同一像素取最大覆盖度，
//! 重叠处不会越描越深），最后一次性合成到帧上：画笔按覆盖度叠加颜色，橡皮按覆盖度降低透明度。
//! 圆边有 1 像素抗锯齿。

use image::RgbaImage;
use serde::Deserialize;
use tf_plugin_api::PluginResult;

use crate::frame::{blend, error, parse_color};

/// 笔刷直径范围（像素）
pub const MIN_SIZE: f32 = 1.0;
pub const MAX_SIZE: f32 = 64.0;
/// 一笔最多的采样点数
pub const MAX_POINTS: usize = 10_000;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Brush,
    Eraser,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    pub tool: Tool,
    /// `#rrggbb`，橡皮忽略
    #[serde(default)]
    pub color: String,
    /// 笔刷直径（像素）
    pub size: f32,
    /// 画布像素坐标 [x, y]
    pub points: Vec<[f32; 2]>,
}

/// 把一笔画到帧上
pub fn apply(image: &mut RgbaImage, stroke: &Stroke) -> PluginResult<()> {
    if stroke.points.is_empty() || stroke.points.len() > MAX_POINTS {
        return Err(error("bad_stroke"));
    }
    if !stroke.size.is_finite() || !(MIN_SIZE..=MAX_SIZE).contains(&stroke.size) {
        return Err(error("bad_stroke"));
    }
    if stroke
        .points
        .iter()
        .any(|[x, y]| !x.is_finite() || !y.is_finite())
    {
        return Err(error("bad_stroke"));
    }
    let color = match stroke.tool {
        Tool::Brush => parse_color(&stroke.color)?,
        Tool::Eraser => [0, 0, 0],
    };
    let (w, h) = image.dimensions();
    let mut coverage = vec![0f32; (w * h) as usize];
    let radius = stroke.size / 2.0;
    let step = (radius * 0.3).max(0.5);
    // 指针拖出画布时坐标可能很远；限制在画布外一圈内，避免沿超长线段盖章耗尽时间
    let margin = MAX_SIZE;
    let clamp = |[x, y]: [f32; 2]| {
        [
            x.clamp(-margin, w as f32 + margin),
            y.clamp(-margin, h as f32 + margin),
        ]
    };
    let points: Vec<[f32; 2]> = stroke.points.iter().copied().map(clamp).collect();
    let mut prev = points[0];
    stamp(&mut coverage, w, h, prev, radius);
    for &point in &points[1..] {
        let (dx, dy) = (point[0] - prev[0], point[1] - prev[1]);
        let length = (dx * dx + dy * dy).sqrt();
        let steps = (length / step).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            stamp(
                &mut coverage,
                w,
                h,
                [prev[0] + dx * t, prev[1] + dy * t],
                radius,
            );
        }
        prev = point;
    }
    for (pixel, &cov) in image.pixels_mut().zip(&coverage) {
        if cov <= 0.0 {
            continue;
        }
        match stroke.tool {
            Tool::Brush => blend(pixel, color, cov),
            Tool::Eraser => {
                let a = f32::from(pixel[3]) * (1.0 - cov);
                pixel[3] = a.round().clamp(0.0, 255.0) as u8;
                if pixel[3] == 0 {
                    *pixel = image::Rgba([0, 0, 0, 0]);
                }
            }
        }
    }
    Ok(())
}

/// 在覆盖度缓冲里盖一个圆头印章（像素中心到圆心的距离决定覆盖度，边缘 1 像素过渡）
fn stamp(coverage: &mut [f32], w: u32, h: u32, center: [f32; 2], radius: f32) {
    let [cx, cy] = center;
    let reach = radius + 1.0;
    let x0 = (cx - reach).floor().max(0.0) as i64;
    let y0 = (cy - reach).floor().max(0.0) as i64;
    let x1 = ((cx + reach).ceil() as i64).min(i64::from(w) - 1);
    let y1 = ((cy + reach).ceil() as i64).min(i64::from(h) - 1);
    // 细笔（直径 < 1）也至少覆盖到中心像素
    let r = radius.max(0.5);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let cov = (r + 0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                let slot = &mut coverage[(y as u32 * w + x as u32) as usize];
                if cov > *slot {
                    *slot = cov;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "stroke_test.rs"]
mod tests;
