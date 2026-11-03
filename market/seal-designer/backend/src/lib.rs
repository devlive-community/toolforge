//! 文人印章设计器后端：姓名/闲章文字排成印面 SVG（方形/长方/圆形、朱文白文、
//! 二至四字布局、镜像预览）。文字用查看端系统字体渲染，导出可带透明背景。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct SealDesigner {
    manifest: Manifest,
}

impl Default for SealDesigner {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for SealDesigner {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "render" => to_value(render(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(SealDesigner);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 印面文字（2–4 字）
    pub text: String,
    /// square / rect / round
    #[serde(default)]
    pub shape: Option<String>,
    /// zhuvwen（朱文，红字白底）/ baiwen（白文，红底白字）
    #[serde(default)]
    pub style: Option<String>,
    /// 印面边长（毫米，20–120）
    #[serde(default)]
    pub size_mm: Option<f64>,
    /// 边框粗细（毫米，0.5–5）
    #[serde(default)]
    pub border_mm: Option<f64>,
    /// 镜像（刻印预览）
    #[serde(default)]
    pub mirror: bool,
    /// 背景透明（false 时输出白底）
    #[serde(default)]
    pub transparent: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub svg: String,
    pub chars: Vec<String>,
    pub shape: String,
    pub style: String,
    /// 阅读顺序（从右到左、右上起）的字序
    pub order: Vec<String>,
}

pub fn render(args: Args) -> PluginResult<Output> {
    let chars: Vec<String> = args.text.trim().chars().map(|c| c.to_string()).collect();
    if !(2..=4).contains(&chars.len()) {
        return Err(PluginError::new("seal.bad_char_count").with("count", chars.len() as u64));
    }
    let shape = args.shape.unwrap_or_else(|| "square".into());
    if !matches!(shape.as_str(), "square" | "rect" | "round") {
        return Err(PluginError::new("seal.bad_shape").with("shape", shape.clone()));
    }
    let style = args.style.unwrap_or_else(|| "zhuwen".into());
    if !matches!(style.as_str(), "zhuwen" | "baiwen") {
        return Err(PluginError::new("seal.bad_style").with("style", style.clone()));
    }
    let size = args.size_mm.unwrap_or(40.0);
    if !(20.0..=120.0).contains(&size) {
        return Err(PluginError::new("seal.bad_size").with("size", size));
    }
    let border = args.border_mm.unwrap_or(2.0);
    if !(0.5..=5.0).contains(&border) {
        return Err(PluginError::new("seal.bad_border").with("border", border));
    }
    // 印文阅读序 = 原文顺序；位置按右上 → 右下 → 左上 → 左下摆放
    let order: Vec<String> = chars.clone();
    // 布局：2 字右列上下；3 字右列上中 + 左下；4 字两列（右上/右下/左上/左下）
    let positions: Vec<(f64, f64)> = match order.len() {
        2 => vec![(0.75, 0.28), (0.75, 0.74)],
        3 => vec![(0.75, 0.28), (0.75, 0.74), (0.25, 0.74)],
        _ => vec![(0.75, 0.28), (0.75, 0.74), (0.25, 0.28), (0.25, 0.74)],
    };
    let width = if shape == "rect" { size * 0.72 } else { size };
    let red = "#b91c1c";
    let face_fill = if style == "baiwen" { red } else { "none" };
    let text_fill = if style == "baiwen" { "#ffffff" } else { red };
    let font_size = size / (if order.len() >= 3 { 2.6 } else { 2.0 });
    let mut svg = String::with_capacity(1024);
    svg.push_str(&format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{:.1}mm' height='{:.1}mm' viewBox='0 0 {:.1} {:.1}'>"#,
        width, size, width, size
    ));
    if !args.transparent {
        svg.push_str(&format!(
            r#"<rect width='{:.1}' height='{:.1}' fill='#ffffff'/>"#,
            width, size
        ));
    }
    // 印面边框
    let half = border / 2.0;
    if shape == "round" {
        svg.push_str(&format!(
            r#"<circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='{}' stroke='{}' stroke-width='{:.1}'/>"#,
            width / 2.0, size / 2.0, size / 2.0 - half, face_fill, red, border
        ));
    } else {
        svg.push_str(&format!(
            r#"<rect x='{half:.1}' y='{half:.1}' width='{:.1}' height='{:.1}' fill='{}' stroke='{}' stroke-width='{:.1}' rx='{}'/>"#,
            width - border, size - border, face_fill, red, border, if shape == "rect" { size * 0.06 } else { 0.0 }
        ));
    }
    // 文字（镜像用 transform scale(-1,1)）
    let mirror_open = if args.mirror {
        format!(r#"<g transform='translate({:.1},0) scale(-1,1)'>"#, width)
    } else {
        "<g>".into()
    };
    svg.push_str(&mirror_open);
    for (index, ch) in order.iter().enumerate() {
        let (fx, fy) = positions[index];
        svg.push_str(&format!(
            r#"<text x='{:.1}' y='{:.1}' text-anchor='middle' dominant-baseline='central' font-family='serif' font-size='{:.1}' font-weight='bold' fill='{}'>{}</text>"#,
            fx * width, fy * size, font_size, text_fill, ch
        ));
    }
    svg.push_str("</g></svg>");
    // 阅读顺序即原文顺序；order 已按印面位置（右上→右下→左上→左下）映射渲染
    let read_order: Vec<String> = order.clone();
    Ok(Output {
        svg,
        chars,
        shape: shape.clone(),
        style: style.clone(),
        order: read_order,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
