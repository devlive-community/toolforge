//! 主色提取：缩小图片后在 OKLab 空间做 k-means（k-means++ 初始化、固定随机种子，结果可复现），
//! 合并过于接近的颜色，按占比排序。

use image::RgbaImage;
use image::imageops::{self, FilterType};
use serde::Serialize;

use crate::color::{self, Formats, Lab};

/// 参与聚类的最多像素（缩小后约 200×200）
const SAMPLE: u32 = 200;
const ITERATIONS: usize = 24;
/// 两个颜色在 OKLab 中的距离平方小于它时合并
const MERGE: f32 = 0.0009;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Swatch {
    /// [r, g, b]
    #[serde(rename = "channels")]
    pub rgb: [u8; 3],
    /// 占不透明像素的比例
    pub share: f32,
    #[serde(flatten)]
    pub formats: Formats,
}

/// 简单的确定性随机数（xorshift）
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn samples(image: &RgbaImage) -> Vec<Lab> {
    let (w, h) = image.dimensions();
    let scale = (SAMPLE as f32 / w.max(h) as f32).min(1.0);
    let small;
    let image = if scale < 1.0 {
        small = imageops::resize(
            image,
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
            // 最近邻采样：只取图中真实存在的颜色，避免边缘插值出不存在的过渡色
            FilterType::Nearest,
        );
        &small
    } else {
        image
    };
    image
        .pixels()
        // 跳过大部分透明的像素（如 PNG 的透明背景）
        .filter(|p| p[3] >= 128)
        .map(|p| color::rgb_to_oklab([p[0], p[1], p[2]]))
        .collect()
}

/// k-means++ 初始化：每个新中心按与已有中心的距离加权抽取
fn seeds(points: &[Lab], k: usize, rng: &mut Rng) -> Vec<Lab> {
    let mut centers = vec![points[(rng.next() * points.len() as f32) as usize % points.len()]];
    let mut nearest: Vec<f32> = points
        .iter()
        .map(|p| color::distance(p, &centers[0]))
        .collect();
    while centers.len() < k {
        let total: f32 = nearest.iter().sum();
        if total <= f32::EPSILON {
            break;
        }
        let mut target = rng.next() * total;
        let index = nearest
            .iter()
            .position(|d| {
                target -= d;
                target <= 0.0
            })
            .unwrap_or(points.len() - 1);
        let center = points[index];
        for (d, p) in nearest.iter_mut().zip(points) {
            *d = d.min(color::distance(p, &center));
        }
        centers.push(center);
    }
    centers
}

pub fn extract(image: &RgbaImage, k: usize) -> Vec<Swatch> {
    let points = samples(image);
    if points.is_empty() {
        return Vec::new();
    }
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut centers = seeds(&points, k.min(points.len()), &mut rng);
    let mut assignment = vec![0usize; points.len()];
    for _ in 0..ITERATIONS {
        let mut changed = false;
        for (i, p) in points.iter().enumerate() {
            let best = centers
                .iter()
                .enumerate()
                .min_by(|a, b| color::distance(p, a.1).total_cmp(&color::distance(p, b.1)))
                .map(|(i, _)| i)
                .unwrap_or(0);
            changed |= assignment[i] != best;
            assignment[i] = best;
        }
        let mut sums = vec![([0f32; 3], 0usize); centers.len()];
        for (p, &c) in points.iter().zip(&assignment) {
            for (sum, v) in sums[c].0.iter_mut().zip(p) {
                *sum += v;
            }
            sums[c].1 += 1;
        }
        for (center, (sum, n)) in centers.iter_mut().zip(&sums) {
            if *n > 0 {
                *center = [sum[0] / *n as f32, sum[1] / *n as f32, sum[2] / *n as f32];
            }
        }
        if !changed {
            break;
        }
    }
    let mut groups: Vec<(Lab, usize)> = centers
        .iter()
        .enumerate()
        .map(|(i, c)| (*c, assignment.iter().filter(|&&a| a == i).count()))
        .filter(|(_, n)| *n > 0)
        .collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.1));
    // 合并几乎相同的颜色（加权平均）
    let mut merged: Vec<(Lab, usize)> = Vec::new();
    for (center, n) in groups {
        if let Some(existing) = merged
            .iter_mut()
            .find(|(c, _)| color::distance(c, &center) < MERGE)
        {
            let total = (existing.1 + n) as f32;
            let weight = existing.1 as f32;
            for (value, c) in existing.0.iter_mut().zip(center) {
                *value = (*value * weight + c * n as f32) / total;
            }
            existing.1 += n;
        } else {
            merged.push((center, n));
        }
    }
    merged.sort_by_key(|g| std::cmp::Reverse(g.1));
    let total = points.len() as f32;
    merged
        .into_iter()
        .map(|(center, n)| {
            let rgb = color::oklab_to_rgb(center);
            Swatch {
                rgb,
                share: n as f32 / total,
                formats: color::formats(rgb),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "palette_test.rs"]
mod tests;
