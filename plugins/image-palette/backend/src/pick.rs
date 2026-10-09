//! 取色：读取某个像素（或周围方块的平均色），并返回放大镜用的周围像素。

use image::RgbaImage;
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::color::{self, Formats};

/// 放大镜的边长（奇数，中心为所取的像素）
pub const LOUPE: i64 = 11;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Picked {
    pub x: u32,
    pub y: u32,
    /// [r, g, b]
    #[serde(rename = "channels")]
    pub rgb: [u8; 3],
    /// 透明度 0–255（取平均时为平均值）
    pub alpha: u8,
    #[serde(flatten)]
    pub formats: Formats,
    /// 放大镜：LOUPE×LOUPE 个十六进制颜色，超出图片的位置为 null
    pub loupe: Vec<Option<String>>,
}

/// radius 为 0 时取单个像素，否则取 (2r+1)² 方块的平均色
pub fn pick(image: &RgbaImage, x: u32, y: u32, radius: u32) -> PluginResult<Picked> {
    let (w, h) = image.dimensions();
    if x >= w || y >= h {
        return Err(PluginError::new("palette.out_of_bounds"));
    }
    let r = radius.min(25) as i64;
    let (mut sum, mut count) = ([0u64; 4], 0u64);
    for dy in -r..=r {
        for dx in -r..=r {
            let (px, py) = (x as i64 + dx, y as i64 + dy);
            if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                continue;
            }
            let p = image.get_pixel(px as u32, py as u32);
            for c in 0..4 {
                sum[c] += p[c] as u64;
            }
            count += 1;
        }
    }
    let avg = |c: usize| (sum[c] as f64 / count as f64).round() as u8;
    let rgb = [avg(0), avg(1), avg(2)];
    let half = LOUPE / 2;
    let loupe = (-half..=half)
        .flat_map(|dy| (-half..=half).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| {
            let (px, py) = (x as i64 + dx, y as i64 + dy);
            (px >= 0 && py >= 0 && px < w as i64 && py < h as i64).then(|| {
                let p = image.get_pixel(px as u32, py as u32);
                color::hex([p[0], p[1], p[2]])
            })
        })
        .collect();
    Ok(Picked {
        x,
        y,
        rgb,
        alpha: avg(3),
        formats: color::formats(rgb),
        loupe,
    })
}

#[cfg(test)]
#[path = "pick_test.rs"]
mod tests;
