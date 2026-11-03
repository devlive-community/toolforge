//! 纸盒与信封展开图后端：插口长方盒与四瓣信封的几何计算与 SVG 生成。
//! 所有尺寸以毫米为单位；线型：裁切线实线、折叠线虚线、粘贴区斜线填充。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct PaperBoxTemplates {
    manifest: Manifest,
}

impl Default for PaperBoxTemplates {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PaperBoxTemplates {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "box" => to_value(box_template(parse_args(args)?)?),
            "envelope" => to_value(envelope_template(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PaperBoxTemplates);

fn positive(value: f64, name: &str) -> PluginResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(PluginError::new("template.positive_required").with("name", name));
    }
    Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxArgs {
    /// 成品内长（毫米）
    pub length: f64,
    /// 成品内宽
    pub width: f64,
    /// 成品内高
    pub height: f64,
    /// 粘边宽度（毫米，默认 15）
    #[serde(default)]
    pub glue: Option<f64>,
    /// 插舌深度（毫米，默认 12）
    #[serde(default)]
    pub tab: Option<f64>,
    /// 纸张长边（毫米，默认 A4 297）
    #[serde(default)]
    pub paper_long: Option<f64>,
    /// 纸张短边（毫米，默认 210）
    #[serde(default)]
    pub paper_short: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateOutput {
    /// 是否放进指定纸张
    pub fits: bool,
    /// 展开图整体尺寸（毫米）
    pub sheet_long: f64,
    pub sheet_short: f64,
    pub svg: String,
    /// 组装步骤（i18n 键序列 steps.<key>）
    pub steps: Vec<&'static str>,
}

/// 插口长方盒展开图：主体面 + 侧折片 + 上下插舌盖
pub fn box_template(args: BoxArgs) -> PluginResult<TemplateOutput> {
    let length = positive(args.length, "length")?;
    let width = positive(args.width, "width")?;
    let height = positive(args.height, "height")?;
    let glue = args.glue.unwrap_or(15.0);
    let tab = args.tab.unwrap_or(12.0);
    for (value, name) in [(glue, "glue"), (tab, "tab")] {
        positive(value, name)?;
    }
    if !(5.0..=40.0).contains(&glue) || !(5.0..=40.0).contains(&tab) {
        return Err(PluginError::new("template.bad_tab_glue"));
    }
    if length > 500.0 || width > 500.0 || height > 500.0 {
        return Err(PluginError::new("template.too_large"));
    }
    // 展开图： glue | H | L | H | L | glue 横排；高度 glue + W + tab 上下（沿 W 方向折）
    let sheet_long = glue + height + length + height + length + glue;
    let sheet_short = glue + width + tab + glue;
    let paper_long = args.paper_long.unwrap_or(297.0);
    let paper_short = args.paper_short.unwrap_or(210.0);
    positive(paper_long, "paperLong")?;
    positive(paper_short, "paperShort")?;
    let fits = sheet_long <= paper_long + 0.01 && sheet_short <= paper_short + 0.01;
    let mut svg = String::with_capacity(2048);
    svg.push_str(&format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{:.1}mm' height='{:.1}mm' viewBox='0 0 {:.1} {:.1}'>"#,
        sheet_long, sheet_short, sheet_long, sheet_short
    ));
    svg.push_str(r#"<rect width='100%' height='100%' fill='#ffffff'/>"#);
    // 折叠线（虚线）：竖线 x = glue+H, glue+H+L, glue+H+L+H
    let x0 = glue;
    let mut xs = vec![glue];
    for delta in [height, length, height, length] {
        let next = xs.last().unwrap() + delta;
        xs.push(next);
    }
    let y_top = glue;
    let y_bottom = glue + width;
    for x in &xs[1..xs.len() - 1] {
        svg.push_str(&format!(
            r#"<line x1="{x:.1}" y1="{y_top:.1}" x2="{x:.1}" y2="{y_bottom:.1}" stroke='#2563eb' stroke-width='0.3' stroke-dasharray='2 1.5'/>"#
        ));
    }
    // 主体外框（裁切线）：x0..x5, y_top..y_bottom
    let x5 = *xs.last().unwrap();
    svg.push_str(&format!(
        r#"<rect x="{x0:.1}" y="{y_top:.1}" width="{:.1}" height="{:.1}" fill='none' stroke='#111827' stroke-width='0.3'/>"#,
        x5 - x0,
        width
    ));
    // 顶盖（插舌）：主体顶部第 2/4 片上方
    for &cover_x in &[xs[1], xs[3]] {
        let cover_w = length;
        svg.push_str(&format!(
            r#"<path d="M{cover_x:.1} {y_top:.1} V{:.1} H{:.1} V{y_top:.1}" fill='none' stroke='#111827' stroke-width='0.3'/>"#,
            y_top - tab,
            cover_x + cover_w
        ));
        // 插舌斜角
        svg.push_str(&format!(
            r#"<path d="M{cover_x:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z" fill='none' stroke='#111827' stroke-width='0.3'/>"#,
            y_top - tab,
            cover_x + cover_w / 2.0,
            y_top - tab - 3.0,
            cover_x + cover_w,
            y_top - tab
        ));
    }
    // 底盖折片（第 2/4 片下方，与顶盖交错对折）
    for &flap_x in &[xs[1], xs[3]] {
        svg.push_str(&format!(
            r#"<path d="M{flap_x:.1} {y_bottom:.1} V{:.1} H{:.1} V{y_bottom:.1}" fill='none' stroke='#111827' stroke-width='0.3'/>"#,
            y_bottom + glue * 0.6,
            flap_x + length
        ));
    }
    // 粘边（首尾 glue 条，斜线填充示意）
    for &glue_x in &[0.0, x5] {
        svg.push_str(&format!(
            r#"<rect x="{glue_x:.1}" y="{y_top:.1}" width="{glue:.1}" height="{width:.1}" fill='url(#hatch)' stroke='#111827' stroke-width='0.3'/>"#
        ));
    }
    svg.push_str(r#"<defs><pattern id="hatch" width='3' height='3' patternTransform='rotate(45)' patternUnits='userSpaceOnUse'><line x1='0' y1='0' x2='0' y2='3' stroke='#9ca3af' stroke-width='0.5'/></pattern></defs>"#);
    svg.push_str("</svg>");
    let steps = vec!["cut", "fold_sides", "fold_flaps", "glue_side", "insert_tab"];
    Ok(TemplateOutput {
        fits,
        sheet_long,
        sheet_short,
        svg,
        steps,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeArgs {
    /// 卡片长（毫米）
    pub card_length: f64,
    /// 卡片宽（毫米）
    pub card_width: f64,
    /// 装入余量（毫米，默认 3）
    #[serde(default)]
    pub clearance: Option<f64>,
}

/// 四瓣信封：中央方形（卡宽）+ 四个梯形瓣
pub fn envelope_template(args: EnvelopeArgs) -> PluginResult<TemplateOutput> {
    let card_length = positive(args.card_length, "cardLength")?;
    let card_width = positive(args.card_width, "cardWidth")?;
    let clearance = args.clearance.unwrap_or(3.0);
    positive(clearance, "clearance")?;
    if clearance > 20.0 || card_length > 500.0 || card_width > 500.0 {
        return Err(PluginError::new("template.too_large"));
    }
    // 中央放卡片宽的正方区域；四瓣展开后整体约 (2.6 × card) 见方
    let center = card_width + clearance;
    let petal = card_length * 0.62;
    let sheet = center + petal * 2.0;
    let fits = sheet <= 297.0;
    let mut svg = String::with_capacity(1024);
    svg.push_str(&format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{:.1}mm' height='{:.1}mm' viewBox='0 0 {:.1} {:.1}'>"#,
        sheet, sheet, sheet, sheet
    ));
    svg.push_str(r#"<rect width='100%' height='100%' fill='#ffffff'/>"#);
    let cx = sheet / 2.0;
    let half = center / 2.0;
    // 中央兜
    svg.push_str(&format!(
        r#"<rect x="{:.1}" y="{:.1}" width="{center:.1}" height="{center:.1}" fill='none' stroke='#111827' stroke-width='0.3'/>"#,
        cx - half,
        cx - half
    ));
    // 四瓣（上/下/左/右梯形），虚线为折线（中央矩形边）
    let petals = [
        (
            format!(
                "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z",
                cx - half,
                cx - half,
                cx,
                cx - half - petal,
                cx + half,
                cx - half,
                cx + half,
                cx - half
            ),
            "top",
        ),
        (
            format!(
                "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z",
                cx - half,
                cx + half,
                cx,
                cx + half + petal,
                cx + half,
                cx + half,
                cx + half,
                cx - half
            ),
            "bottom",
        ),
        (
            format!(
                "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z",
                cx - half,
                cx - half,
                cx - half - petal,
                cx,
                cx - half,
                cx + half,
                cx - half,
                cx + half
            ),
            "left",
        ),
        (
            format!(
                "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z",
                cx + half,
                cx - half,
                cx + half + petal,
                cx,
                cx + half,
                cx + half,
                cx + half,
                cx + half
            ),
            "right",
        ),
    ];
    for (path, label) in petals {
        svg.push_str(&format!(
            r#"<path d="{path}" fill='none' stroke='#111827' stroke-width='0.3'/>"#
        ));
        svg.push_str(&format!(
            r#"<text x='0' y='0' font-size='0' data-flap="{label}"></text>"#
        ));
    }
    svg.push_str("</svg>");
    let steps = vec![
        "cut",
        "fold_center",
        "fold_bottom",
        "fold_top",
        "fold_side",
        "tuck",
    ];
    Ok(TemplateOutput {
        fits,
        sheet_long: sheet,
        sheet_short: sheet,
        svg,
        steps,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
