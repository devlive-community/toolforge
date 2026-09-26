use super::*;

fn area(r: &Rect) -> f64 {
    r.w * r.h
}

#[test]
fn fills_the_rectangle_proportionally() {
    let values = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 600.0,
        h: 400.0,
    };
    let rects = squarify(&values, rect);
    assert_eq!(rects.len(), values.len());
    let total: f64 = rects.iter().map(area).sum();
    assert!((total - 240_000.0).abs() < 1e-6);
    for (value, r) in values.iter().zip(&rects) {
        assert!((area(r) - value / 24.0 * 240_000.0).abs() < 1e-6);
        assert!(
            r.x >= -1e-9 && r.y >= -1e-9 && r.x + r.w <= 600.0 + 1e-6 && r.y + r.h <= 400.0 + 1e-6,
            "{r:?}"
        );
    }
    // 方形化：长宽比都不会太极端
    for r in &rects {
        let ratio = (r.w / r.h).max(r.h / r.w);
        assert!(ratio < 4.0, "{r:?}");
    }
}

#[test]
fn handles_edge_cases() {
    let rect = Rect {
        x: 10.0,
        y: 20.0,
        w: 100.0,
        h: 50.0,
    };
    assert!(squarify(&[], rect).is_empty());
    assert_eq!(squarify(&[5.0], rect), vec![rect]);
    let zero = squarify(&[0.0, 0.0], rect);
    assert!(zero.iter().all(|r| r.w == 0.0 && r.h == 0.0));
    let empty = squarify(&[1.0], Rect { w: 0.0, ..rect });
    assert_eq!(empty[0].w, 0.0);
}
