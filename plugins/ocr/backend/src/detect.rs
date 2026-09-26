//! 文字检测（DB 模型）的前后处理：按比例缩放为 32 的倍数，概率图二值化后取连通区域，
//! 按 DB 的 unclip 规则向外扩展，再映射回原图坐标并按阅读顺序排序。

use image::RgbImage;
use image::imageops::FilterType;
use serde::Serialize;
use tract_onnx::prelude::*;

/// 检测输入的最长边；截图中的小字需要较高的分辨率
pub const MAX_SIDE: u32 = 1920;
const THRESHOLD: f32 = 0.3;
const BOX_THRESHOLD: f32 = 0.6;
const UNCLIP_RATIO: f32 = 1.5;
const MIN_SIDE: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(skip)]
    pub score: f32,
}

/// 检测输入尺寸（宽、高，均为 32 的倍数）
pub fn input_size(width: u32, height: u32) -> (u32, u32) {
    let scale = (MAX_SIDE as f32 / width.max(height) as f32).min(1.0);
    let round = |v: u32| (((v as f32 * scale) / 32.0).round() as u32).max(1) * 32;
    (round(width), round(height))
}

/// BGR 顺序，ImageNet 均值方差归一化（与 PaddleOCR 一致）
pub fn input(image: &RgbImage, width: u32, height: u32) -> Tensor {
    let resized = image::imageops::resize(image, width, height, FilterType::Triangle);
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];
    tract_ndarray::Array4::from_shape_fn((1, 3, height as usize, width as usize), |(_, c, y, x)| {
        let pixel = resized.get_pixel(x as u32, y as u32).0;
        (pixel[2 - c] as f32 / 255.0 - MEAN[c]) / STD[c]
    })
    .into()
}

/// 从概率图提取文字区域（原图坐标）
pub fn regions(
    prob: &[f32],
    map_w: usize,
    map_h: usize,
    image_w: u32,
    image_h: u32,
) -> Vec<Region> {
    let (sx, sy) = (image_w as f32 / map_w as f32, image_h as f32 / map_h as f32);
    let mut seen = vec![false; prob.len()];
    let mut stack = Vec::new();
    let mut found = Vec::new();
    for start in 0..prob.len() {
        if seen[start] || prob[start] <= THRESHOLD {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        let (mut sum, mut count) = (0.0f32, 0usize);
        while let Some(index) = stack.pop() {
            let (x, y) = (index % map_w, index / map_w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            sum += prob[index];
            count += 1;
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= map_w as i64 || ny >= map_h as i64 {
                        continue;
                    }
                    let next = ny as usize * map_w + nx as usize;
                    if !seen[next] && prob[next] > THRESHOLD {
                        seen[next] = true;
                        stack.push(next);
                    }
                }
            }
        }
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let score = sum / count as f32;
        if w.min(h) < MIN_SIDE || score < BOX_THRESHOLD {
            continue;
        }
        // DB 的收缩区域需要按「面积 × 比例 / 周长」向外扩展
        let distance = (w * h) as f32 * UNCLIP_RATIO / (2 * (w + h)) as f32;
        let left = (x0 as f32 - distance).max(0.0);
        let top = (y0 as f32 - distance).max(0.0);
        let right = ((x1 + 1) as f32 + distance).min(map_w as f32);
        let bottom = ((y1 + 1) as f32 + distance).min(map_h as f32);
        found.push(Region {
            x: left * sx,
            y: top * sy,
            w: (right - left) * sx,
            h: (bottom - top) * sy,
            score,
        });
    }
    order(found)
}

/// 阅读顺序：按行（垂直方向重叠过半视为同一行）从上到下，行内从左到右
pub fn order(mut regions: Vec<Region>) -> Vec<Region> {
    regions.sort_by(|a, b| (a.y + a.h / 2.0).total_cmp(&(b.y + b.h / 2.0)));
    let mut rows: Vec<Vec<Region>> = Vec::new();
    for region in regions {
        let same_row = rows.last().is_some_and(|row| {
            let last = row[row.len() - 1];
            let overlap = (last.y + last.h).min(region.y + region.h) - last.y.max(region.y);
            overlap > last.h.min(region.h) * 0.5
        });
        match rows.last_mut() {
            Some(row) if same_row => row.push(region),
            _ => rows.push(vec![region]),
        }
    }
    rows.into_iter()
        .flat_map(|mut row| {
            row.sort_by(|a, b| a.x.total_cmp(&b.x));
            row
        })
        .collect()
}

/// 同一行的判断，用于把区域拼成文本
pub fn same_row(a: &Region, b: &Region) -> bool {
    let overlap = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    overlap > a.h.min(b.h) * 0.5
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
