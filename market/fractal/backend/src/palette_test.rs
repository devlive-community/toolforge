use super::*;

#[test]
fn palettes_cover_and_interpolate() {
    for name in palettes() {
        // 首尾与中点都有效
        let start = sample(name, 0.0, Brightness::Normal);
        let end = sample(name, 1.0, Brightness::Normal);
        let mid = sample(name, 0.5, Brightness::Normal);
        for color in [start, end, mid] {
            assert!(color.iter().all(|v| (0..=255).contains(v)));
        }
        assert_ne!(start, end, "palette {name} should span a gradient");
    }
    // 未知调色板回退到第一个
    assert_eq!(
        sample("nope", 0.0, Brightness::Normal),
        sample(SPECTRUM, 0.0, Brightness::Normal)
    );
    // 位置越界 clamp
    assert_eq!(
        sample(SPECTRUM, -1.0, Brightness::Normal),
        sample(SPECTRUM, 0.0, Brightness::Normal)
    );
    assert_eq!(
        sample(SPECTRUM, 2.0, Brightness::Normal),
        sample(SPECTRUM, 1.0, Brightness::Normal)
    );
}

#[test]
fn brightness_adjusts_channels() {
    let base = sample(EMBER, 0.8, Brightness::Normal);
    let bright = sample(EMBER, 0.8, Brightness::Bright);
    let dim = sample(EMBER, 0.8, Brightness::Dim);
    assert!(bright.iter().zip(base.iter()).all(|(b, v)| b >= v));
    assert!(dim.iter().zip(base.iter()).all(|(b, v)| b <= v));
}
