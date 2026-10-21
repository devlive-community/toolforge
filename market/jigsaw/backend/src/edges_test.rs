use super::*;

#[test]
fn generation_is_deterministic_and_well_formed() {
    let a = generate(6, 6, 42);
    let b = generate(6, 6, 42);
    assert_eq!(a.eh, b.eh, "same seed must reproduce edges");
    assert_eq!(a.ev.len(), 6);
    assert_eq!(a.eh.len(), 5);
    assert_eq!(a.eh[0].len(), 6);
    assert_eq!(a.ev[0].len(), 5);
    // 参数在约定范围内，且方向只取 ±1
    for grid in [&a.eh, &a.ev] {
        for row in grid.iter() {
            for edge in row {
                assert_eq!(edge[0].abs(), 1.0, "sign must be ±1");
                assert!((0.4..=0.6).contains(&edge[1]), "mid jitter {}", edge[1]);
                assert!((0.17..=0.25).contains(&edge[2]), "height {}", edge[2]);
            }
        }
    }
    // 两种方向都会出现（6x6 有 60 条边）
    let mut signs = std::collections::HashSet::new();
    for row in &a.eh {
        for edge in row {
            signs.insert(edge[0] as i8);
        }
    }
    assert!(signs.contains(&1) && signs.contains(&-1));
    // 不同种子应产生不同结果
    let c = generate(6, 6, 43);
    assert_ne!(a.eh, c.eh);
    // 边界规格：3x3 只有 4 条边
    let small = generate(3, 3, 7);
    assert_eq!(small.eh.len(), 2);
    assert_eq!(small.ev.len(), 3);
    assert_eq!(small.ev[0].len(), 2);
}

#[test]
fn judge_snaps_within_tolerance_only() {
    let (cw, ch) = (100.0_f64, 80.0_f64);
    // 容差 = 0.3 * min(cw, ch) = 24
    // 目标位置 (200, 0)：距离 0 → 吸附
    let (placed, x, y) = judge(0, 2, 201.0, 2.0, cw, ch);
    assert!(placed);
    assert_eq!((x, y), (200.0, 0.0));
    // 距离恰好在容差边缘内
    let (placed, x, y) = judge(0, 2, 200.0 + 23.0, 0.0, cw, ch);
    assert!(placed);
    assert_eq!((x, y), (200.0, 0.0));
    // 超出容差 → 保留原位
    let (placed, x, y) = judge(0, 2, 200.0 + 25.0, 0.0, cw, ch);
    assert!(!placed);
    assert_eq!((x, y), (225.0, 0.0));
    // 对角远离
    let (placed, x, y) = judge(1, 1, 500.0, 500.0, cw, ch);
    assert!(!placed);
    assert_eq!((x, y), (500.0, 500.0));
}
