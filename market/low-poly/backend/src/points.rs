//! 特征点生成：Sobel 边缘强度 + 均匀网格抖动，确定性种子。
//! 四角锚点固定；接受率 = 基础均匀 + detail 加权的边缘强度。

use image::RgbaImage;

/// 灰度 Sobel 梯度幅值（0-255 近似，未归一化到 8 位）
fn edge_map(image: &RgbaImage) -> Vec<f32> {
    let (w, h) = (image.width(), image.height());
    let gray = |x: u32, y: u32| -> f32 {
        let p = image.get_pixel(x.min(w - 1), y.min(h - 1));
        let [r, g, b, _] = p.0;
        0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)
    };
    let mut map = vec![0.0f32; w as usize * h as usize];
    for y in 0..h {
        for x in 0..w {
            // Sobel：边界 clamp
            let tl = gray(x.saturating_sub(1), y.saturating_sub(1));
            let t = gray(x, y.saturating_sub(1));
            let tr = gray((x + 1).min(w - 1), y.saturating_sub(1));
            let l = gray(x.saturating_sub(1), y);
            let r = gray((x + 1).min(w - 1), y);
            let bl = gray(x.saturating_sub(1), (y + 1).min(h - 1));
            let b = gray(x, (y + 1).min(h - 1));
            let br = gray((x + 1).min(w - 1), (y + 1).min(h - 1));
            let gx = tl + 2.0 * l + bl - tr - 2.0 * r - br;
            let gy = tl + 2.0 * t + tr - bl - 2.0 * b - br;
            map[y as usize * w as usize + x as usize] = (gx * gx + gy * gy).sqrt();
        }
    }
    map
}

/// xorshift64：确定性随机
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn unit(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

/// 生成点集：四角锚点 + 边缘加权散点，点间最小间距约束
pub fn generate(image: &RgbaImage, seed: u64, count: usize, detail: f64) -> Vec<[f64; 2]> {
    let (w, h) = (f64::from(image.width()), f64::from(image.height()));
    let mut rng = Rng::new(seed);
    let mut out = vec![
        [0.0, 0.0],
        [w - 1.0, 0.0],
        [w - 1.0, h - 1.0],
        [0.0, h - 1.0],
    ];
    if count <= out.len() {
        return out;
    }
    let edges = edge_map(image);
    // 归一化边缘强度 → 接受权重
    let max_edge = edges.iter().cloned().fold(0.0f32, f32::max).max(1.0);
    let min_gap = ((w * h).sqrt() / (count as f64 * 0.8)).max(2.0);
    let inner = count - out.len();
    // 候选尝试次数（够密即可，拒绝采样）
    let attempts = inner * 24;
    for _ in 0..attempts {
        if out.len() >= count {
            break;
        }
        let x = rng.unit() * (w - 1.0);
        let y = rng.unit() * (h - 1.0);
        let edge = edges[((y as u32).min(image.height() - 1) * image.width()
            + (x as u32).min(image.width() - 1)) as usize];
        let weight = edge / max_edge;
        // detail=0 纯均匀，detail=1 边缘优先；接受概率 = mix(0.15, weight, detail)
        let accept = 0.15 + (1.0 - 0.15) * detail * f64::from(weight);
        if rng.unit() > accept {
            continue;
        }
        // 最小间距：与已有点保持距离，避免退化细三角
        let gap2 = min_gap * min_gap;
        if out.iter().any(|p| {
            let dx = p[0] - x;
            let dy = p[1] - y;
            dx * dx + dy * dy < gap2
        }) {
            continue;
        }
        out.push([x, y]);
    }
    out
}

#[cfg(test)]
#[path = "points_test.rs"]
mod tests;
