//! 拼片边缘：每条内部边一份形状参数（凹凸方向、中点位置与高度抖动）。
//! 相邻两片共用同一绝对曲线，天然互相吻合；形状曲线本身由前端绘制，
//! 后端只负责生成参数与按位置判定吸附 / 完成保证一致性。

use serde::{Deserialize, Serialize};

/// 一条边的形状参数：sign=+1 表示凸片属于左/上片（凸起伸向右/下），
/// m 为凸起中点沿边位置（0-1），h 为凸起高度（占边长比例）。
pub type EdgeSpec = [f64; 3];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeGrid {
    pub cols: u32,
    pub rows: u32,
    /// 水平内边（上片与下片之间）：eh[r][c]，r ∈ 0..rows-1
    pub eh: Vec<Vec<EdgeSpec>>,
    /// 垂直内边（左片与右片之间）：ev[r][c]，c ∈ 0..cols-1
    pub ev: Vec<Vec<EdgeSpec>>,
}

/// xorshift64：同种子确定性生成
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// 生成全部内部边参数；jitter 控制中点与高度的随机幅度
pub fn generate(cols: u32, rows: u32, seed: u64) -> EdgeGrid {
    let mut rng = Rng(seed.max(1));
    let mut eh = Vec::with_capacity((rows as usize).saturating_sub(1));
    for _ in 0..rows.saturating_sub(1) {
        let mut row = Vec::with_capacity(cols as usize);
        for _ in 0..cols {
            row.push(spec(&mut rng));
        }
        eh.push(row);
    }
    let mut ev = Vec::with_capacity(rows as usize);
    for _ in 0..rows {
        let mut row = Vec::with_capacity((cols as usize).saturating_sub(1));
        for _ in 0..cols.saturating_sub(1) {
            row.push(spec(&mut rng));
        }
        ev.push(row);
    }
    EdgeGrid { cols, rows, eh, ev }
}

fn spec(rng: &mut Rng) -> EdgeSpec {
    let sign = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
    // 中点在 0.5 附近 ±0.05，高度 0.18-0.24 边长
    [sign, 0.45 + rng.unit() * 0.1, 0.18 + rng.unit() * 0.06]
}

/// 吸附判定：拼片格位起点与目标格位距离在容差内则吸附到精确位置。
/// 判定只依赖格位几何与身份，不按画面相似度猜测。
pub fn judge(row: u32, col: u32, x: f64, y: f64, cell_w: f64, cell_h: f64) -> (bool, f64, f64) {
    let target_x = f64::from(col) * cell_w;
    let target_y = f64::from(row) * cell_h;
    let dx = x - target_x;
    let dy = y - target_y;
    let tolerance = 0.3 * cell_w.min(cell_h);
    if dx * dx + dy * dy <= tolerance * tolerance {
        (true, target_x, target_y)
    } else {
        (false, x, y)
    }
}

#[cfg(test)]
#[path = "edges_test.rs"]
mod tests;
