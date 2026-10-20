//! 平方迭代：Mandelbrot（z0=0, c=像素）与 Julia（z0=像素, c=参数）。
//! 明确使用有限精度与有限迭代：达到上限未逃逸只标注「本次未逃逸」。

/// 平滑迭代值：逃逸时返回 Some(n + 1 - log2(log|z|))；未逃逸返回 None
#[inline]
pub fn escape(
    kind: Kind,
    cx: f64,
    cy: f64,
    julia_x: f64,
    julia_y: f64,
    max_iter: u32,
) -> Option<f64> {
    let (mut zx, mut zy) = match kind {
        Kind::Mandelbrot => (0.0, 0.0),
        Kind::Julia => (cx, cy),
    };
    let (px, py) = match kind {
        Kind::Mandelbrot => (cx, cy),
        Kind::Julia => (julia_x, julia_y),
    };
    let mut n = 0u32;
    // |z|² > 4 即逃逸；平方比较避免开方
    while zx * zx + zy * zy <= 4.0 && n < max_iter {
        let next_x = zx * zx - zy * zy + px;
        zy = 2.0 * zx * zy + py;
        zx = next_x;
        n += 1;
    }
    if n >= max_iter {
        return None;
    }
    // 平滑：n + 1 - log2(log2(|z|²)/2)
    let modulus_sq = zx * zx + zy * zy;
    let smooth = n as f64 + 1.0 - (0.5 * modulus_sq.ln()).log2();
    Some(smooth.max(0.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mandelbrot,
    Julia,
}

/// 双精度可用的视角宽度下限：低于此值坐标增量小于有效尾数，画面只会重复
pub const PRECISION_LIMIT: f64 = 1e-13;

#[cfg(test)]
#[path = "fractal_test.rs"]
mod tests;
