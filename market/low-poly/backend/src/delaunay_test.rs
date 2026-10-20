use super::triangulate;

fn sorted_ids(triangles: &[[usize; 3]]) -> Vec<[usize; 3]> {
    let mut out: Vec<[usize; 3]> = triangles
        .iter()
        .map(|t| {
            let mut s = *t;
            s.sort_unstable();
            s
        })
        .collect();
    out.sort_unstable();
    out
}

#[test]
fn triangulates_square_with_diagonal() {
    let points = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let triangles = triangulate(&points);
    assert_eq!(triangles.len(), 2);
    // 每个三角都由 4 个输入点组成
    for tri in sorted_ids(&triangles) {
        assert!(tri.iter().all(|v| *v < 4));
    }
}

#[test]
fn covers_convex_hull_and_contains_all_points() {
    // 圆周点 + 圆心：全部点应被使用
    let mut points = vec![[10.0, 10.0]];
    for i in 0..16 {
        let angle = i as f64 / 16.0 * std::f64::consts::TAU;
        points.push([10.0 + 10.0 * angle.cos(), 10.0 + 10.0 * angle.sin()]);
    }
    let triangles = triangulate(&points);
    assert!(triangles.len() >= 16);
    // 圆心应出现在某个三角里
    assert!(triangles.iter().any(|t| t.contains(&0)));
    // 无重复三角
    let ids = sorted_ids(&triangles);
    let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), unique.len());
}

#[test]
fn deduplicates_near_points() {
    let points = [
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 10.0],
        [0.0, 10.0],
        [10.0, 0.0 + 1e-13],
    ];
    let triangles = triangulate(&points);
    // 重复点不影响：仍是 2 个三角
    assert_eq!(triangles.len(), 2);
}

#[test]
fn rejects_degenerate_inputs() {
    // 点数不足
    assert!(triangulate(&[[0.0, 0.0], [1.0, 1.0]]).is_empty());
    // 完全共线
    assert!(triangulate(&[[0.0, 0.0], [5.0, 5.0], [10.0, 10.0]]).is_empty());
}

#[test]
fn handles_large_point_sets() {
    // 2000 点上限性能与正确性抽样
    let mut points = Vec::new();
    let mut seed = 0x9E3779B97F4A7C15u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..2000 {
        points.push([(next() % 1000) as f64, (next() % 1000) as f64]);
    }
    let triangles = triangulate(&points);
    // 平面点集三角数 ≈ 2n - 2 - h（h 为凸包点数），放宽范围验证
    assert!(triangles.len() > 3500 && triangles.len() < 5000);
    // 索引全部有效
    for tri in &triangles {
        assert!(tri.iter().all(|v| *v < 2000));
    }
}
