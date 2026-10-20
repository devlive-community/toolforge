//! Delaunay 三角剖分（Bowyer-Watson）：确定性、无外部依赖。
//! 点数上限 2000，插入法 O(n·T)；重复点（<1e-9）被剔除，全共线点集返回
//! 空三角列表由调用方报错（生成路径固定含四角锚点，正常不会触发）。

/// 空圆判定的相对 epsilon：点数与坐标都在千级，绝对值足够
const EPS: f64 = 1e-9;

/// 外接圆判定：点 p 是否在三角形 (a,b,c) 的外接圆内
fn in_circumcircle(a: [f64; 2], b: [f64; 2], c: [f64; 2], p: [f64; 2]) -> bool {
    let ax = a[0] - p[0];
    let ay = a[1] - p[1];
    let bx = b[0] - p[0];
    let by = b[1] - p[1];
    let cx = c[0] - p[0];
    let cy = c[1] - p[1];
    let det = (ax * ax + ay * ay) * (bx * cy - by * cx) - (bx * bx + by * by) * (ax * cy - ay * cx)
        + (cx * cx + cy * cy) * (ax * by - ay * bx);
    // 逆时针三角形内为正；顶点顺序可能被翻转，按带符号面积取方向
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area > 0.0 {
        det > EPS * scale(a, b, c)
    } else if area < 0.0 {
        det < -EPS * scale(a, b, c)
    } else {
        false
    }
}

/// 判定量级（坐标平方和），让 epsilon 与尺度成正比
fn scale(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let s = (a[0] * a[0] + a[1] * a[1] + b[0] * b[0] + b[1] * b[1] + c[0] * c[0] + c[1] * c[1])
        .max(1.0);
    s * s
}

/// 剖分：输入点集（会被去重；顺序不变），返回三角索引
pub fn triangulate(input: &[[f64; 2]]) -> Vec<[usize; 3]> {
    if input.len() < 3 {
        return vec![];
    }
    // 去重：距离过近的点视为同一点，索引映射到首个出现位置
    let mut points: Vec<[f64; 2]> = Vec::with_capacity(input.len());
    let mut index = vec![usize::MAX; input.len()];
    for (i, p) in input.iter().enumerate() {
        let mut found = None;
        for (j, q) in points.iter().enumerate() {
            let dx = q[0] - p[0];
            let dy = q[1] - p[1];
            if dx * dx + dy * dy < 1e-12 {
                found = Some(j);
                break;
            }
        }
        match found {
            Some(j) => index[i] = j,
            None => {
                index[i] = points.len();
                points.push(*p);
            }
        }
    }
    if points.len() < 3 {
        return vec![];
    }

    // 超三角形：完全覆盖输入包围盒
    let (min_x, max_x, min_y, max_y) = points.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(x0, x1, y0, y1), p| (x0.min(p[0]), x1.max(p[0]), y0.min(p[1]), y1.max(p[1])),
    );
    let dx = (max_x - min_x).max(1.0);
    let dy = (max_y - min_y).max(1.0);
    let mid = [(min_x + max_x) / 2.0, (min_y + max_y) / 2.0];
    let sup = [
        [mid[0] - 20.0 * dx, mid[1] - dy],
        [mid[0] + 20.0 * dx, mid[1] - dy],
        [mid[0], mid[1] + 20.0 * dy],
    ];
    points.extend_from_slice(&sup);
    let (s0, s1, s2) = (points.len() - 3, points.len() - 2, points.len() - 1);

    // 逐点插入
    let mut triangles: Vec<[usize; 3]> = vec![[s0, s1, s2]];
    for (i, p) in points.iter().enumerate().take(points.len() - 3) {
        // 找外接圆包含该点的三角形
        let mut bad = vec![false; triangles.len()];
        for (t, tri) in triangles.iter().enumerate() {
            if in_circumcircle(points[tri[0]], points[tri[1]], points[tri[2]], *p) {
                bad[t] = true;
            }
        }
        // bad 三角形的边界边（在 bad 集合中只出现一次的边）
        let mut boundary: Vec<[usize; 2]> = Vec::new();
        for (t, tri) in triangles.iter().enumerate() {
            if !bad[t] {
                continue;
            }
            let [a, b, c] = *tri;
            for edge in [[a, b], [b, c], [c, a]] {
                // 与已收集边比较（无向）
                if let Some(pos) = boundary.iter().position(|e| {
                    (e[0] == edge[0] && e[1] == edge[1]) || (e[0] == edge[1] && e[1] == edge[0])
                }) {
                    // 出现两次：内部边，移除
                    boundary.swap_remove(pos);
                } else {
                    boundary.push(edge);
                }
            }
        }
        // 删除 bad 三角形（保留原顺序）
        let mut kept = Vec::with_capacity(triangles.len());
        for (t, tri) in triangles.drain(..).enumerate() {
            if !bad[t] {
                kept.push(tri);
            }
        }
        triangles = kept;
        // 连接边界边与新点
        for edge in &boundary {
            triangles.push([edge[0], edge[1], i]);
        }
    }

    // 去掉含超顶点的三角（其余索引不变：超顶点都在数组尾部）
    triangles
        .into_iter()
        .filter(|tri| tri.iter().all(|v| *v < s0))
        .collect()
}

#[cfg(test)]
#[path = "delaunay_test.rs"]
mod tests;
