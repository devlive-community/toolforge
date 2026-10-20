use super::{Kind, escape};
use crate::palette;
use crate::render::{self};

fn input(kind: Kind, width: u32, height: u32) -> render::RenderInput<'static> {
    render::RenderInput {
        kind,
        center_x: -0.5,
        center_y: 0.0,
        span_x: 4.0,
        julia_x: -0.8,
        julia_y: 0.156,
        width,
        height,
        max_iter: 256,
        palette: "spectrum",
        brightness: palette::Brightness::Normal,
        progress: None,
        is_cancelled: None,
    }
}

/// 已知测试点：Mandelbrot 中 c=-1 属于集合（周期 2），c=2 立即逃逸
#[test]
fn mandelbrot_known_points() {
    let interior = escape(Kind::Mandelbrot, -1.0, 0.0, 0.0, 0.0, 256);
    assert_eq!(interior, None, "c = -1 should stay bounded");
    let escaped = escape(Kind::Mandelbrot, 2.0, 0.0, 0.0, 0.0, 256);
    let value = escaped.expect("c = 2 escapes immediately");
    assert!(value < 3.0, "c = 2 escapes in 2 steps, got {value}");
    // 原点（c=0）永不逃逸
    assert_eq!(escape(Kind::Mandelbrot, 0.0, 0.0, 0.0, 0.0, 256), None);
}

/// Julia：c=0 时迭代是纯平方，|z|<1 不逃逸、|z|>1 逃逸
#[test]
fn julia_known_points() {
    assert_eq!(escape(Kind::Julia, 0.5, 0.0, 0.0, 0.0, 256), None);
    let escaped = escape(Kind::Julia, 1.5, 0.0, 0.0, 0.0, 256);
    let value = escaped.expect("|z|>1 escapes");
    assert!(value < 3.0);
}

#[test]
fn renders_without_stretching_across_ratios() {
    // 宽高比不同：spanY 按比例换算，画面不被拉伸——
    // 同一视角下中心行的集合横截面占比（filled/width）应当一致
    let fractions: Vec<f64> = [(200u32, 200u32), (300u32, 150u32)]
        .iter()
        .map(|&(width, height)| {
            let out = render::render(&input(Kind::Mandelbrot, width, height)).unwrap();
            // 扫描中心行，数非黑（逃逸着色）像素
            let row = height / 2;
            let filled = (0..width)
                .filter(|&col| {
                    let p = out.image.get_pixel(col, row).0;
                    p[0] > 0 || p[1] > 0 || p[2] > 0
                })
                .count();
            filled as f64 / f64::from(width)
        })
        .collect();
    let diff = (fractions[0] - fractions[1]).abs();
    assert!(
        diff < 0.02,
        "cross-section fraction must match across ratios: {fractions:?}"
    );
    // 全景下中心行逃逸区占比合理（不是全黑也不是全彩）
    for fraction in fractions {
        assert!(fraction > 0.3 && fraction < 0.8, "fraction {fraction}");
    }
}

#[test]
fn reports_precision_limit_and_interior_ratio() {
    // 极深 zoom：span 低于精度极限仍渲染，但标注
    let mut view = input(Kind::Mandelbrot, 40, 40);
    view.span_x = 1e-15;
    let out = render::render(&view).unwrap();
    assert!(out.stats.precision_limited);
    // 全景：主心形未逃逸像素占比在合理范围（约 10-25%）
    let out = render::render(&input(Kind::Mandelbrot, 100, 100)).unwrap();
    assert!(out.stats.interior_ratio > 0.05 && out.stats.interior_ratio < 0.35);
}
