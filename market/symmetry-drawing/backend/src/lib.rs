//! 对称涂鸦画板后端：把用户笔画按镜像/旋转对称展开为完整 SVG。
//! 实时绘制由前端 canvas 完成；导出 SVG 的几何展开在 Rust，保证撤销分组与
//! 对称参数随每条笔画保存。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct SymmetryDrawing {
    manifest: Manifest,
}

impl Default for SymmetryDrawing {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for SymmetryDrawing {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "export_svg" => {
                let args: ExportArgs = parse_args(args)?;
                match export_svg(&args) {
                    Ok(svg) => serde_json::to_value(&Exported { svg }).map_err(|e| {
                        PluginError::new("plugin.serialize_failed").with("detail", e.to_string())
                    }),
                    Err(err) => Err(err),
                }
            }
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(SymmetryDrawing);

/// 一条笔画：折线点 + 该笔画自己的对称配置
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    /// 点序列 [x0, y0, x1, y1, ...]
    pub points: Vec<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub opacity: Option<f64>,
    /// none / mirror_x / mirror_y / mirror_xy / rotate
    #[serde(default)]
    pub mode: Option<String>,
    /// 旋转对称份数（rotate 模式 2–16）
    #[serde(default)]
    pub folds: Option<usize>,
    /// 对称中心
    #[serde(default)]
    pub center: Option<(f64, f64)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub strokes: Vec<Stroke>,
    pub width: f64,
    pub height: f64,
    /// 背景色；transparent 时不输出背景
    #[serde(default)]
    pub background: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub svg: String,
}

const MAX_POINTS: usize = 20_000;
const MAX_STROKES: usize = 500;

pub fn export_svg(args: &ExportArgs) -> PluginResult<String> {
    if args.width <= 0.0 || args.height <= 0.0 || args.width > 10_000.0 || args.height > 10_000.0 {
        return Err(PluginError::new("symmetry.bad_canvas"));
    }
    if args.strokes.len() > MAX_STROKES {
        return Err(PluginError::new("symmetry.too_many_strokes").with("max", MAX_STROKES as u64));
    }
    let total_points: usize = args.strokes.iter().map(|s| s.points.len()).sum();
    if total_points > MAX_POINTS {
        return Err(PluginError::new("symmetry.too_many_points").with("max", MAX_POINTS as u64));
    }
    let mut out = String::with_capacity(4096);
    out.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
        args.width, args.height, args.width, args.height
    ));
    match args.background.as_deref() {
        None | Some("transparent") => {}
        Some(color) => {
            validate_color(color)?;
            out.push_str(&format!(
                r#"<rect width="100%" height="100%" fill="{color}"/>"#
            ));
        }
    }
    for stroke in &args.strokes {
        let points = &stroke.points;
        if points.len() < 4 || points.len() % 2 != 0 {
            return Err(PluginError::new("symmetry.bad_stroke"));
        }
        if points.iter().any(|p| !p.is_finite()) {
            return Err(PluginError::new("symmetry.bad_stroke"));
        }
        let color = stroke.color.clone().unwrap_or_else(|| "#334155".into());
        validate_color(&color)?;
        let width = stroke.width.unwrap_or(3.0);
        if !(0.1..=60.0).contains(&width) {
            return Err(PluginError::new("symmetry.bad_width").with("width", width));
        }
        let opacity = stroke.opacity.unwrap_or(1.0);
        if !(0.0..=1.0).contains(&opacity) {
            return Err(PluginError::new("symmetry.bad_opacity").with("opacity", opacity));
        }
        let mode = stroke.mode.as_deref().unwrap_or("none");
        let folds = stroke.folds.unwrap_or(1);
        let (cx, cy) = stroke
            .center
            .unwrap_or((args.width / 2.0, args.height / 2.0));
        // 生成所有对称副本的折线
        let copies = copies(points, mode, folds, cx, cy)?;
        for copy in &copies {
            let mut d = String::with_capacity(copy.len() * 8);
            for (index, (x, y)) in copy.iter().enumerate() {
                d.push_str(&if index == 0 {
                    format!("M{x:.2} {y:.2}")
                } else {
                    format!(" L{x:.2} {y:.2}")
                });
            }
            out.push_str(&format!(
                r#"<path d="{d}" fill="none" stroke="{color}" stroke-width="{width:.2}" stroke-opacity="{opacity:.2}" stroke-linecap="round" stroke-linejoin="round"/>"#
            ));
        }
    }
    out.push_str("</svg>");
    Ok(out)
}

/// 计算一条笔画的所有对称副本
fn copies(
    points: &[f64],
    mode: &str,
    folds: usize,
    cx: f64,
    cy: f64,
) -> PluginResult<Vec<Vec<(f64, f64)>>> {
    let source: Vec<(f64, f64)> = points.chunks(2).map(|c| (c[0], c[1])).collect();
    let mut out = Vec::new();
    match mode {
        "none" => out.push(source),
        "mirror_x" => {
            out.push(source.clone());
            out.push(source.iter().map(|(x, y)| (*x, 2.0 * cy - y)).collect());
        }
        "mirror_y" => {
            out.push(source.clone());
            out.push(source.iter().map(|(x, y)| (2.0 * cx - x, *y)).collect());
        }
        "mirror_xy" => {
            out.push(source.clone());
            out.push(source.iter().map(|(x, y)| (2.0 * cx - x, *y)).collect());
            out.push(source.iter().map(|(x, y)| (*x, 2.0 * cy - y)).collect());
            out.push(
                source
                    .iter()
                    .map(|(x, y)| (2.0 * cx - x, 2.0 * cy - y))
                    .collect(),
            );
        }
        "rotate" => {
            if !(2..=16).contains(&folds) {
                return Err(PluginError::new("symmetry.bad_folds").with("folds", folds as u64));
            }
            let angle = std::f64::consts::TAU / folds as f64;
            for fold in 0..folds {
                let theta = angle * fold as f64;
                let (sin, cos) = theta.sin_cos();
                out.push(
                    source
                        .iter()
                        .map(|(x, y)| {
                            let dx = x - cx;
                            let dy = y - cy;
                            (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
                        })
                        .collect(),
                );
            }
        }
        other => return Err(PluginError::new("symmetry.bad_mode").with("mode", other)),
    }
    Ok(out)
}

fn validate_color(color: &str) -> PluginResult<()> {
    let ok = (color.starts_with('#')
        && color.len() <= 9
        && color[1..].chars().all(|c| c.is_ascii_hexdigit()))
        || color == "transparent"
        || color == "none";
    if ok {
        Ok(())
    } else {
        Err(PluginError::new("symmetry.bad_color").with("color", color))
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
