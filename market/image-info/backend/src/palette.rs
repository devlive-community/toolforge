//! 主色提取：缩小后的像素做确定性 k-means（最远点初始化），按像素占比排序。

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use serde::Serialize;

const SAMPLE: u32 = 96;
const ITERATIONS: usize = 12;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Swatch {
    pub hex: String,
    /// 0-100
    pub percent: f64,
}

fn hex(c: [f64; 3]) -> String {
    let [r, g, b] = c.map(|v| v.round().clamp(0.0, 255.0) as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    // 近似感知权重（红绿蓝对亮度的贡献不同）
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db
}

/// 返回主色（按占比降序）与平均色
pub fn extract(image: &DynamicImage, k: usize) -> (Vec<Swatch>, Option<String>) {
    let small = if image.width().max(image.height()) > SAMPLE {
        image.resize(SAMPLE, SAMPLE, FilterType::Triangle)
    } else {
        image.clone()
    };
    let pixels: Vec<[f64; 3]> = small
        .pixels()
        .filter(|(_, _, p)| p.0[3] >= 128)
        .map(|(_, _, p)| [p.0[0] as f64, p.0[1] as f64, p.0[2] as f64])
        .collect();
    if pixels.is_empty() {
        return (Vec::new(), None);
    }
    let n = pixels.len() as f64;
    let average = pixels.iter().fold([0.0; 3], |acc, p| {
        [acc[0] + p[0] / n, acc[1] + p[1] / n, acc[2] + p[2] / n]
    });

    // 最远点初始化：从平均色最远的像素开始，依次挑选距离已选中心最远的像素
    let mut centers: Vec<[f64; 3]> = Vec::new();
    let mut nearest: Vec<f64> = pixels.iter().map(|p| distance(*p, average)).collect();
    while centers.len() < k.max(1) {
        let (index, best) =
            nearest.iter().enumerate().fold(
                (0, -1.0),
                |acc, (i, d)| if *d > acc.1 { (i, *d) } else { acc },
            );
        if best <= 0.0 && !centers.is_empty() {
            break;
        }
        let center = pixels[index];
        centers.push(center);
        for (d, p) in nearest.iter_mut().zip(&pixels) {
            *d = d.min(distance(*p, center));
        }
    }

    let mut counts = vec![0usize; centers.len()];
    for _ in 0..ITERATIONS {
        let mut sums = vec![[0.0; 3]; centers.len()];
        counts.iter_mut().for_each(|c| *c = 0);
        for p in &pixels {
            let (index, _) = centers
                .iter()
                .enumerate()
                .map(|(i, c)| (i, distance(*p, *c)))
                .fold((0, f64::MAX), |acc, x| if x.1 < acc.1 { x } else { acc });
            counts[index] += 1;
            for ch in 0..3 {
                sums[index][ch] += p[ch];
            }
        }
        for (i, center) in centers.iter_mut().enumerate() {
            if counts[i] > 0 {
                *center = sums[i].map(|s| s / counts[i] as f64);
            }
        }
    }

    let mut swatches: Vec<(usize, [f64; 3])> = counts
        .into_iter()
        .zip(centers)
        .filter(|(count, _)| *count > 0)
        .collect();
    swatches.sort_by_key(|s| std::cmp::Reverse(s.0));
    (
        swatches
            .into_iter()
            .map(|(count, color)| Swatch {
                hex: hex(color),
                percent: (count as f64 / n * 1000.0).round() / 10.0,
            })
            .collect(),
        Some(hex(average)),
    )
}

#[cfg(test)]
#[path = "palette_test.rs"]
mod tests;
