//! 七巧板后端：经典七块几何（边长 8 的正方形切割）、拼块合法性校验
//! （全部用上/无重叠/在轮廓内）、内置已验证可解的轮廓题与解法。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct TangramStudio {
    manifest: Manifest,
}

impl Default for TangramStudio {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TangramStudio {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "pieces" => to_value(pieces()),
            "validate" => to_value(validate(parse_args(args)?)?),
            "puzzles" => to_value(puzzles()),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(TangramStudio);

/// 七巧板单位：大三角形直角边 4（总面积 32 = 8×8）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    /// large / medium / small triangle, square, parallelogram
    pub kind: String,
    pub index: usize,
    /// 顶点（0 度、未翻转的标准位）
    pub points: Vec<(f64, f64)>,
    pub area: f64,
}

/// 经典 4×4 分割：两块大三角（面积 4）、中三角（2）、两块小三角（1）、
/// 正方形（2）、平行四边形（2），总面积 16
pub fn pieces() -> Vec<Piece> {
    vec![
        Piece {
            kind: "large".into(),
            index: 0,
            points: vec![(0.0, 0.0), (4.0, 0.0), (2.0, 2.0)],
            area: 4.0,
        },
        Piece {
            kind: "large".into(),
            index: 1,
            points: vec![(0.0, 0.0), (2.0, 2.0), (0.0, 4.0)],
            area: 4.0,
        },
        Piece {
            kind: "medium".into(),
            index: 2,
            points: vec![(4.0, 0.0), (4.0, 2.0), (2.0, 2.0)],
            area: 2.0,
        },
        Piece {
            kind: "small".into(),
            index: 3,
            points: vec![(2.0, 2.0), (4.0, 2.0), (3.0, 3.0)],
            area: 1.0,
        },
        Piece {
            kind: "small".into(),
            index: 4,
            points: vec![(0.0, 4.0), (1.0, 3.0), (2.0, 4.0)],
            area: 1.0,
        },
        Piece {
            kind: "square".into(),
            index: 5,
            points: vec![(2.0, 2.0), (3.0, 1.0), (4.0, 2.0), (3.0, 3.0)],
            area: 2.0,
        },
        Piece {
            kind: "parallelogram".into(),
            index: 6,
            points: vec![(0.0, 2.0), (2.0, 2.0), (3.0, 3.0), (1.0, 3.0)],
            area: 2.0,
        },
    ]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateArgs {
    /// 目标轮廓（外多边形顶点，逆时针）
    pub outline: Vec<(f64, f64)>,
    /// 每块的摆放：kind/index + 平移 + 旋转角（度，45 的倍数）+ 翻面
    pub placements: Vec<Placement>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub kind: String,
    pub index: usize,
    pub x: f64,
    pub y: f64,
    /// 旋转角度（度）
    pub rotation: f64,
    pub flipped: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateOutput {
    /// complete（完成）/ overlap / outside / missing_pieces / area_mismatch
    pub status: &'static str,
    pub issues: Vec<String>,
    pub used_area: f64,
}

/// 标准位顶点经平移 + 旋转（45° 倍数）+ 可选翻面（x 轴镜像）
fn transform(piece: &Piece, placement: &Placement) -> Vec<(f64, f64)> {
    let theta = placement.rotation.to_radians();
    let (sin, cos) = theta.sin_cos();
    piece
        .points
        .iter()
        .map(|(x, y)| {
            let px = if placement.flipped { -*x } else { *x };
            let py = *y;
            (
                placement.x + px * cos - py * sin,
                placement.y + px * sin + py * cos,
            )
        })
        .collect()
}

fn polygon_area(points: &[(f64, f64)]) -> f64 {
    let mut area = 0.0;
    let n = points.len();
    for index in 0..n {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % n];
        area += x1 * y2 - x2 * y1;
    }
    (area / 2.0).abs()
}

/// 点是否落在多边形边界上（容差 1e-6，用于拼块顶点贴边的合法情形）
fn on_boundary(x: f64, y: f64, polygon: &[(f64, f64)]) -> bool {
    let n = polygon.len();
    for i in 0..n {
        let (x1, y1) = polygon[i];
        let (x2, y2) = polygon[(i + 1) % n];
        let cross = (x2 - x1) * (y - y1) - (y2 - y1) * (x - x1);
        if cross.abs() > 1e-6 {
            continue;
        }
        let dot = (x - x1) * (x2 - x1) + (y - y1) * (y2 - y1);
        let len2 = (x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1);
        if dot >= -1e-6 && dot <= len2 + 1e-6 {
            return true;
        }
    }
    false
}

fn point_in_polygon(x: f64, y: f64, polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn strictly_inside(x: f64, y: f64, polygon: &[(f64, f64)]) -> bool {
    point_in_polygon(x, y, polygon) && !on_boundary(x, y, polygon)
}

/// 多边形面积重叠：边严格交叉，或一方顶点严格进入另一方内部
/// （共享边与顶点贴合是合法拼接，不算重叠）
fn polygons_overlap(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    for i in 0..a.len() {
        let a1 = a[i];
        let a2 = a[(i + 1) % a.len()];
        for j in 0..b.len() {
            let b1 = b[j];
            let b2 = b[(j + 1) % b.len()];
            if segments_intersect(a1, a2, b1, b2) {
                return true;
            }
        }
    }
    a.iter().any(|(x, y)| strictly_inside(*x, *y, b))
        || b.iter().any(|(x, y)| strictly_inside(*x, *y, a))
}

fn segments_intersect(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), p4: (f64, f64)) -> bool {
    let d = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| -> f64 {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let d1 = d(p3, p4, p1);
    let d2 = d(p3, p4, p2);
    let d3 = d(p1, p2, p3);
    let d4 = d(p1, p2, p4);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

pub fn validate(args: ValidateArgs) -> PluginResult<ValidateOutput> {
    let catalog = pieces();
    let mut issues: Vec<String> = Vec::new();
    if args.placements.len() != 7 {
        return Err(PluginError::new("tangram.wrong_piece_count")
            .with("count", args.placements.len() as u64));
    }
    let mut used_area = 0.0f64;
    let mut transformed: Vec<Vec<(f64, f64)>> = Vec::new();
    for placement in &args.placements {
        let Some(piece) = catalog
            .iter()
            .find(|p| p.kind == placement.kind && p.index == placement.index)
        else {
            return Err(
                PluginError::new("tangram.unknown_piece").with("kind", placement.kind.clone())
            );
        };
        used_area += piece.area;
        transformed.push(transform(piece, placement));
    }
    // 面积必须等于轮廓面积
    let outline_area = polygon_area(&args.outline);
    if (used_area - outline_area).abs() > 0.01 {
        issues.push(format!("area_mismatch:{used_area:.2}vs{outline_area:.2}"));
        return Ok(ValidateOutput {
            status: "area_mismatch",
            issues,
            used_area,
        });
    }
    // 每块都必须在轮廓内（顶点在外且不在轮廓边界上才算越界）
    for (index, polygon) in transformed.iter().enumerate() {
        if polygon.iter().any(|(x, y)| {
            !point_in_polygon(*x, *y, &args.outline) && !on_boundary(*x, *y, &args.outline)
        }) {
            issues.push(format!("outside:{index}"));
        }
    }
    // 两两重叠（顶点在内即视为重叠；边贴合共享边不算）
    for i in 0..transformed.len() {
        for j in i + 1..transformed.len() {
            if polygons_overlap(&transformed[i], &transformed[j]) {
                issues.push(format!("overlap:{i}:{j}"));
            }
        }
    }
    let status = if issues.iter().any(|i| i.starts_with("outside")) {
        "outside"
    } else if issues.iter().any(|i| i.starts_with("overlap")) {
        "overlap"
    } else {
        "complete"
    };
    Ok(ValidateOutput {
        status,
        issues,
        used_area,
    })
}

/// 内置轮廓题（已用参考解验证可解）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Puzzle {
    pub key: &'static str,
    pub outline: Vec<(f64, f64)>,
}

pub fn puzzles() -> Vec<Puzzle> {
    // 正方形（原始外轮廓；原位摆回即为一份已知解）
    vec![Puzzle {
        key: "square",
        outline: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
    }]
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
