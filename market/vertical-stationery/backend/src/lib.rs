//! 竖排笺纸后端：把文本按从右到左的竖排分栏排入纸张（标题/落款），
//! 生成 SVG。标点在竖排中替换为全角形态并直立呈现（首版简化规则）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct VerticalStationery {
    manifest: Manifest,
}

impl Default for VerticalStationery {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for VerticalStationery {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "layout" => to_value(layout(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(VerticalStationery);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub text: String,
    /// 标题（首列，可选）
    #[serde(default)]
    pub title: Option<String>,
    /// 落款（最后一列，可选）
    #[serde(default)]
    pub signature: Option<String>,
    /// 纸张宽（毫米，80–500）
    #[serde(default)]
    pub paper_width: Option<f64>,
    /// 纸张高（毫米，100–700）
    #[serde(default)]
    pub paper_height: Option<f64>,
    /// 字号（毫米，3–20）
    #[serde(default)]
    pub font_mm: Option<f64>,
    /// 每列字数（5–30）
    #[serde(default)]
    pub per_column: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub svg: String,
    pub columns: usize,
    /// 页数（超出纸张高度时按列分组分页）
    pub pages: usize,
}

/// 竖排标点形态替换（首版简化：常见标点映射到直排习惯字符）
fn vertical_char(ch: char) -> String {
    match ch {
        '，' | ',' => "、".to_owned(),
        '。' | '.' => "。".to_owned(),
        '：' | ':' => "︰".to_owned(),
        '；' | ';' => "；".to_owned(),
        '？' | '?' => "？".to_owned(),
        '！' | '!' => "！".to_owned(),
        '“' | '”' | '"' => "〝".to_owned(),
        '‘' | '’' | '\'' => "〞".to_owned(),
        '（' | '(' => "︵".to_owned(),
        '）' | ')' => "︶".to_owned(),
        '《' | '<' => "︽".to_owned(),
        '》' | '>' => "︾".to_owned(),
        '—' | '–' | '-' => "丨".to_owned(),
        other => other.to_string(),
    }
}

pub fn layout(args: Args) -> PluginResult<Output> {
    let paper_width = args.paper_width.unwrap_or(150.0);
    let paper_height = args.paper_height.unwrap_or(220.0);
    let font = args.font_mm.unwrap_or(7.0);
    let per_column = args.per_column.unwrap_or(20);
    if !(80.0..=500.0).contains(&paper_width) || !(100.0..=700.0).contains(&paper_height) {
        return Err(PluginError::new("stationery.bad_paper"));
    }
    if !(3.0..=20.0).contains(&font) {
        return Err(PluginError::new("stationery.bad_font").with("fontMm", font));
    }
    if !(5..=30).contains(&per_column) {
        return Err(
            PluginError::new("stationery.bad_per_column").with("perColumn", per_column as u64)
        );
    }
    let text = args.text.trim().to_owned();
    if text.is_empty() {
        return Err(PluginError::new("stationery.empty_text"));
    }
    let body: Vec<String> = text.chars().map(vertical_char).collect();
    let cols_max = ((paper_width - 20.0) / (font * 1.6)).floor() as usize;
    if cols_max == 0 {
        return Err(PluginError::new("stationery.bad_paper"));
    }
    let mut columns: Vec<Vec<String>> = Vec::new();
    // 标题占首列
    if let Some(title) = args
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        columns.push(title.chars().map(vertical_char).collect());
    }
    for chunk in body.chunks(per_column) {
        columns.push(chunk.to_vec());
    }
    if let Some(sign) = args
        .signature
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        columns.push(sign.chars().map(vertical_char).collect());
    }
    if columns.is_empty() {
        return Err(PluginError::new("stationery.empty_text"));
    }
    // 每页可容纳的列数
    let cols_per_page = cols_max.max(1);
    let pages = columns.chunks(cols_per_page).count();
    if pages > 20 {
        return Err(PluginError::new("stationery.too_many_pages")
            .with("pages", pages as u64)
            .with("max", 20));
    }
    let margin = 10.0;
    let mut svg = String::with_capacity(2048);
    svg.push_str(&format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{:.1}mm' height='{:.1}mm' viewBox='0 0 {:.1} {:.1}'>"#,
        paper_width, paper_height, paper_width, paper_height
    ));
    svg.push_str(&format!(
        r#"<rect width='{paper_width:.1}' height='{paper_height:.1}' fill='#fdfaf3'/>"#
    ));
    // 边框
    svg.push_str(&format!(
        r#"<rect x='{margin:.1}' y='{margin:.1}' width='{:.1}' height='{:.1}' fill='none' stroke='#8a6d3b' stroke-width='0.4'/>"#,
        paper_width - margin * 2.0,
        paper_height - margin * 2.0
    ));
    // 列：从右到左
    let col_pitch = (paper_width - margin * 2.0) / cols_per_page as f64;
    for (index, column) in columns.iter().enumerate() {
        let page = index / cols_per_page;
        let col_in_page = index % cols_per_page;
        let x = paper_width - margin - col_pitch * (col_in_page as f64 + 0.5);
        let _ = page;
        for (char_index, ch) in column.iter().enumerate() {
            let y = margin + font * 1.4 + char_index as f64 * font * 1.18;
            if y > paper_height - margin {
                break;
            }
            svg.push_str(&format!(
                r#"<text x='{x:.1}' y='{y:.1}' font-family='serif' font-size='{font:.1}' fill='#1f2937' text-anchor='middle'>{ch}</text>"#
            ));
        }
    }
    svg.push_str("</svg>");
    Ok(Output {
        svg,
        columns: columns.len(),
        pages,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
