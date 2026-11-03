//! 几何计算插件后端：常见平面/立体形状求解（附公式与代入过程）与装修用料估算。
//!
//! 所有公式标签（label）返回 i18n 键，由前端翻译；公式本身是数学式原样返回。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::f64::consts::PI;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct GeometryCalculator {
    manifest: Manifest,
}

impl Default for GeometryCalculator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for GeometryCalculator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "solve" => to_value(solve(parse_args(args)?)?),
            "materials" => to_value(materials(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(GeometryCalculator);

/// 形状参数；每个形状只要求提供的字段，缺的按缺失报错
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShapeArgs {
    pub shape: String,
    #[serde(default)]
    pub r: Option<f64>,
    #[serde(default)]
    pub a: Option<f64>,
    #[serde(default)]
    pub b: Option<f64>,
    #[serde(default)]
    pub c: Option<f64>,
    #[serde(default)]
    pub w: Option<f64>,
    #[serde(default)]
    pub l: Option<f64>,
    #[serde(default)]
    pub h: Option<f64>,
}

const SHAPES: &[&str] = &[
    "circle",
    "triangle",
    "rectangle",
    "trapezoid",
    "sphere",
    "cylinder",
    "cone",
    "cuboid",
];

/// 一行结果：value 为数值，formula/substituted 为数学式（不翻译），label 为 i18n 键
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub label: &'static str,
    pub value: f64,
    pub formula: &'static str,
    pub substituted: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveOutput {
    pub shape: String,
    /// 该形状的量纲类别（plane / solid），界面分组显示
    pub kind: &'static str,
    pub rows: Vec<Row>,
}

fn need(key: &str, value: Option<f64>) -> PluginResult<f64> {
    let value =
        value.ok_or_else(|| PluginError::new("geometry.missing_param").with("name", key))?;
    if value <= 0.0 {
        return Err(PluginError::new("geometry.positive_required").with("name", key));
    }
    Ok(value)
}

fn row(label: &'static str, value: f64, formula: &'static str, substituted: String) -> Row {
    Row {
        label,
        value,
        formula,
        substituted,
    }
}

pub fn solve(args: ShapeArgs) -> PluginResult<SolveOutput> {
    if !SHAPES.contains(&args.shape.as_str()) {
        return Err(PluginError::new("geometry.unknown_shape").with("shape", args.shape.clone()));
    }
    let shape = args.shape.as_str();
    let (kind, rows) = match shape {
        "circle" => {
            let r = need("r", args.r)?;
            (
                "plane",
                vec![
                    row(
                        "perimeter",
                        2.0 * PI * r,
                        "C = 2 × π × r",
                        format!("C = 2 × π × {r} = {}", 2.0 * PI * r),
                    ),
                    row(
                        "area",
                        PI * r * r,
                        "A = π × r²",
                        format!("A = π × {r}² = {}", PI * r * r),
                    ),
                ],
            )
        }
        "triangle" => {
            let (a, b, c) = (need("a", args.a)?, need("b", args.b)?, need("c", args.c)?);
            if a + b <= c || a + c <= b || b + c <= a {
                return Err(PluginError::new("geometry.bad_triangle"));
            }
            let perimeter = a + b + c;
            let s = perimeter / 2.0;
            let area = (s * (s - a) * (s - b) * (s - c)).sqrt();
            (
                "plane",
                vec![
                    row(
                        "perimeter",
                        perimeter,
                        "C = a + b + c",
                        format!("C = {a} + {b} + {c} = {perimeter}"),
                    ),
                    row(
                        "area",
                        area,
                        "A = √(s(s−a)(s−b)(s−c)), s = C/2",
                        format!(
                            "s = {s}, A = √({s}×{:.6}×{:.6}×{:.6}) = {}",
                            s - a,
                            s - b,
                            s - c,
                            area
                        ),
                    ),
                ],
            )
        }
        "rectangle" => {
            let (w, l) = (need("w", args.w)?, need("l", args.l)?);
            (
                "plane",
                vec![
                    row(
                        "perimeter",
                        2.0 * (w + l),
                        "C = 2 × (w + l)",
                        format!("C = 2 × ({w} + {l}) = {}", 2.0 * (w + l)),
                    ),
                    row(
                        "area",
                        w * l,
                        "A = w × l",
                        format!("A = {w} × {l} = {}", w * l),
                    ),
                ],
            )
        }
        "trapezoid" => {
            let (a, b, h) = (need("a", args.a)?, need("b", args.b)?, need("h", args.h)?);
            let area = (a + b) * h / 2.0;
            (
                "plane",
                vec![row(
                    "area",
                    area,
                    "A = (a + b) × h / 2",
                    format!("A = ({a} + {b}) × {h} / 2 = {area}"),
                )],
            )
        }
        "sphere" => {
            let r = need("r", args.r)?;
            (
                "solid",
                vec![
                    row(
                        "area",
                        4.0 * PI * r * r,
                        "S = 4 × π × r²",
                        format!("S = 4 × π × {r}² = {}", 4.0 * PI * r * r),
                    ),
                    row(
                        "volume",
                        4.0 / 3.0 * PI * r * r * r,
                        "V = 4 × π × r³ / 3",
                        format!("V = 4 × π × {r}³ / 3 = {}", 4.0 / 3.0 * PI * r * r * r),
                    ),
                ],
            )
        }
        "cylinder" => {
            let (r, h) = (need("r", args.r)?, need("h", args.h)?);
            let side = 2.0 * PI * r * h;
            let area = side + 2.0 * PI * r * r;
            (
                "solid",
                vec![
                    row(
                        "sideArea",
                        side,
                        "S₁ = 2 × π × r × h",
                        format!("S₁ = 2 × π × {r} × {h} = {side}"),
                    ),
                    row(
                        "area",
                        area,
                        "S = S₁ + 2 × π × r²",
                        format!("S = {side} + 2 × π × {r}² = {area}"),
                    ),
                    row(
                        "volume",
                        PI * r * r * h,
                        "V = π × r² × h",
                        format!("V = π × {r}² × {h} = {}", PI * r * r * h),
                    ),
                ],
            )
        }
        "cone" => {
            let (r, h) = (need("r", args.r)?, need("h", args.h)?);
            let slant = (h * h + r * r).sqrt();
            let side = PI * r * slant;
            let area = side + PI * r * r;
            (
                "solid",
                vec![
                    row(
                        "sideArea",
                        side,
                        "S₁ = π × r × √(h² + r²)",
                        format!("S₁ = π × {r} × √({h}² + {r}²) = {side}"),
                    ),
                    row(
                        "area",
                        area,
                        "S = S₁ + π × r²",
                        format!("S = {side} + π × {r}² = {area}"),
                    ),
                    row(
                        "volume",
                        PI * r * r * h / 3.0,
                        "V = π × r² × h / 3",
                        format!("V = π × {r}² × {h} / 3 = {}", PI * r * r * h / 3.0),
                    ),
                ],
            )
        }
        _ => {
            // cuboid
            let (w, l, h) = (need("w", args.w)?, need("l", args.l)?, need("h", args.h)?);
            let area = 2.0 * (w * l + w * h + l * h);
            (
                "solid",
                vec![
                    row(
                        "area",
                        area,
                        "S = 2 × (wl + wh + lh)",
                        format!("S = 2 × ({w}×{l} + {w}×{h} + {l}×{h}) = {area}"),
                    ),
                    row(
                        "volume",
                        w * l * h,
                        "V = w × l × h",
                        format!("V = {w} × {l} × {h} = {}", w * l * h),
                    ),
                ],
            )
        }
    };
    Ok(SolveOutput {
        shape: args.shape,
        kind,
        rows,
    })
}

/// 装修用料估算
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialArgs {
    /// paint（墙面漆）或 tiles（地砖）
    pub kind: String,
    /// 房间长度（米）
    pub length: f64,
    /// 房间宽度（米）
    pub width: f64,
    /// 房间高度（米，paint 用）
    #[serde(default)]
    pub height: Option<f64>,
    /// 门窗等不开刷面积合计（平方米）
    #[serde(default)]
    pub openings: Option<f64>,
    /// 每升漆可涂刷面积（平方米/升，默认 10）
    #[serde(default)]
    pub coverage: Option<f64>,
    /// 涂刷遍数（默认 2）
    #[serde(default)]
    pub coats: Option<f64>,
    /// 地砖长/宽（米，tiles 用）
    #[serde(default)]
    pub tile_length: Option<f64>,
    #[serde(default)]
    pub tile_width: Option<f64>,
    /// 损耗百分比（默认 10）
    #[serde(default)]
    pub waste: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialOutput {
    pub kind: String,
    pub rows: Vec<Row>,
    /// 建议采购量（漆为升，向上取整；砖为块数）
    pub buy: f64,
    /// 采购量单位（i18n 键）
    pub unit: &'static str,
}

pub fn materials(args: MaterialArgs) -> PluginResult<MaterialOutput> {
    if args.length <= 0.0 || args.width <= 0.0 {
        return Err(PluginError::new("geometry.positive_required").with("name", "length/width"));
    }
    let floor = args.length * args.width;
    match args.kind.as_str() {
        "paint" => {
            let height = args
                .height
                .ok_or_else(|| PluginError::new("geometry.missing_param").with("name", "height"))?;
            if height <= 0.0 {
                return Err(PluginError::new("geometry.positive_required").with("name", "height"));
            }
            let openings = args.openings.unwrap_or(0.0);
            let coverage = args.coverage.unwrap_or(10.0);
            let coats = args.coats.unwrap_or(2.0);
            if coverage <= 0.0 || coats <= 0.0 || openings < 0.0 {
                return Err(PluginError::new("geometry.bad_args"));
            }
            let wall = 2.0 * (args.length + args.width) * height - openings;
            let liters = wall * coats / coverage;
            if wall <= 0.0 {
                return Err(PluginError::new("geometry.bad_args"));
            }
            Ok(MaterialOutput {
                kind: args.kind,
                rows: vec![
                    row(
                        "wallArea",
                        wall,
                        "S = 2 × (l + w) × h − 开口",
                        format!(
                            "S = 2 × ({:.2} + {:.2}) × {height} − {openings} = {wall:.2}",
                            args.length, args.width
                        ),
                    ),
                    row(
                        "paintLiters",
                        liters,
                        "Q = S × 遍数 ÷ 覆盖率",
                        format!("Q = {wall:.2} × {coats} ÷ {coverage} = {liters:.2}"),
                    ),
                ],
                buy: liters.ceil(),
                unit: "liters",
            })
        }
        "tiles" => {
            let (tl, tw) = (
                args.tile_length.ok_or_else(|| {
                    PluginError::new("geometry.missing_param").with("name", "tileLength")
                })?,
                args.tile_width.ok_or_else(|| {
                    PluginError::new("geometry.missing_param").with("name", "tileWidth")
                })?,
            );
            let waste = args.waste.unwrap_or(10.0);
            if tl <= 0.0 || tw <= 0.0 || waste < 0.0 {
                return Err(PluginError::new("geometry.bad_args"));
            }
            let tile_area = tl * tw;
            let with_waste = floor * (1.0 + waste / 100.0);
            let pieces = (with_waste / tile_area).ceil();
            Ok(MaterialOutput {
                kind: args.kind,
                rows: vec![
                    row(
                        "floorArea",
                        floor,
                        "S = l × w",
                        format!("S = {:.2} × {:.2} = {floor:.2}", args.length, args.width),
                    ),
                    row(
                        "tilesNeeded",
                        pieces,
                        "N = ⌈S × (1 + 损耗%) ÷ 砖面积⌉",
                        format!("N = ⌈{floor:.2} × (1 + {waste}%) ÷ {tile_area}⌉ = {pieces}"),
                    ),
                ],
                buy: pieces,
                unit: "pieces",
            })
        }
        _ => Err(PluginError::new("geometry.unknown_material").with("kind", args.kind)),
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
