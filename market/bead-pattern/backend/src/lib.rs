//! 拼豆图纸后端：PNG/JPEG 图片 → 限定珠色色表的方格映射（最近色匹配），
//! 输出格色矩阵、每色用量与 SVG 图纸。透明区域可留空或填充。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};
use tf_plugin_guest::net;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct BeadPattern {
    manifest: Manifest,
}

impl Default for BeadPattern {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for BeadPattern {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(BeadPattern);

/// 内置通用示例色表（名称 + RGB）；非某品牌官方色号
pub const PALETTE: &[(&str, u8, u8, u8)] = &[
    ("white", 255, 255, 255),
    ("black", 20, 20, 20),
    ("gray", 128, 128, 128),
    ("lightgray", 211, 211, 211),
    ("red", 220, 40, 40),
    ("darkred", 139, 0, 0),
    ("orange", 255, 140, 0),
    ("yellow", 255, 215, 0),
    ("gold", 218, 165, 32),
    ("green", 34, 139, 34),
    ("lightgreen", 144, 238, 144),
    ("teal", 0, 128, 128),
    ("skyblue", 135, 206, 235),
    ("blue", 30, 90, 200),
    ("navy", 0, 0, 128),
    ("purple", 128, 0, 128),
    ("pink", 255, 150, 190),
    ("brown", 139, 90, 43),
    ("beige", 245, 222, 179),
    ("skin", 255, 220, 180),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 用户选择的图片路径
    pub path: String,
    /// 横向格数（4–100）
    pub columns: usize,
    /// 纵向格数（4–100）
    pub rows: usize,
    /// 最多使用的颜色数（2–20）
    #[serde(default)]
    pub max_colors: Option<usize>,
    /// 透明区域留空（默认填充最近色）
    #[serde(default)]
    pub keep_transparent: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub row: usize,
    pub column: usize,
    /// 色名（PALETTE 键；空格为空串）
    pub color: String,
    /// 黑白打印符号（A–T 按色表序）
    pub symbol: char,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub color: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub columns: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
    pub usage: Vec<Usage>,
    pub total_beads: usize,
    pub svg: String,
}

fn nearest(r: u8, g: u8, b: u8, allowed: &[usize]) -> usize {
    let mut best = allowed[0];
    let mut best_dist = u64::MAX;
    for &index in allowed {
        let (_, pr, pg, pb) = PALETTE[index];
        let dr = r as i32 - pr as i32;
        let dg = g as i32 - pg as i32;
        let db = b as i32 - pb as i32;
        let dist = (dr * dr + dg * dg + db * db) as u64;
        if dist < best_dist {
            best_dist = dist;
            best = index;
        }
    }
    best
}

pub fn generate(args: Args) -> PluginResult<Output> {
    let columns = args.columns;
    let rows = args.rows;
    if !(4..=100).contains(&columns) || !(4..=100).contains(&rows) {
        return Err(PluginError::new("bead.bad_grid")
            .with("columns", columns as u64)
            .with("rows", rows as u64));
    }
    let max_colors = args.max_colors.unwrap_or(12).clamp(2, PALETTE.len());
    let size = net::file_size(&args.path)
        .map_err(|e| PluginError::new("bead.read_failed").with("detail", e.code))?;
    if size > 10 * 1024 * 1024 {
        return Err(PluginError::new("bead.too_large").with("max", 10 * 1024 * 1024));
    }
    let bytes = net::read_to_end(&args.path, 10 * 1024 * 1024)
        .map_err(|e| PluginError::new("bead.read_failed").with("detail", e.code))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|e| PluginError::new("bead.bad_image").with("detail", e.to_string()))?;
    let resized = image::imageops::resize(
        &image.to_rgba8(),
        columns as u32,
        rows as u32,
        image::imageops::FilterType::Triangle,
    );
    // 先统计各格最近色，再按使用频率选出 max_colors 个颜色
    let mut mapped: Vec<Option<usize>> = Vec::with_capacity(columns * rows);
    for pixel in resized.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 64 && args.keep_transparent {
            mapped.push(None);
        } else {
            mapped.push(Some(nearest(
                r,
                g,
                b,
                &(0..PALETTE.len()).collect::<Vec<_>>(),
            )));
        }
    }
    let mut frequency: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for used in mapped.iter().flatten() {
        *frequency.entry(*used).or_insert(0) += 1;
    }
    let mut ranked: Vec<usize> = {
        let mut keys: Vec<usize> = frequency.keys().copied().collect();
        keys.sort_by(|a, b| frequency[b].cmp(&frequency[a]));
        keys
    };
    ranked.truncate(max_colors);
    // 重映射到允许的色表子集
    let allowed = ranked.clone();
    let mut cells: Vec<Cell> = Vec::new();
    let mut usage: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (index, mapped_item) in mapped.iter().enumerate() {
        let (row, column) = (index / columns, index % columns);
        match mapped_item {
            None => cells.push(Cell {
                row,
                column,
                color: String::new(),
                symbol: ' ',
            }),
            Some(palette_index) => {
                let palette_index = *palette_index;
                let final_index = if allowed.contains(&palette_index) {
                    palette_index
                } else {
                    let (_, pr, pg, pb) = PALETTE[palette_index];
                    nearest(pr, pg, pb, &allowed)
                };
                let (name, _, _, _) = PALETTE[final_index];
                let symbol = char::from(b'A' + final_index as u8);
                *usage.entry(name.to_string()).or_insert(0) += 1;
                cells.push(Cell {
                    row,
                    column,
                    color: name.to_string(),
                    symbol,
                });
            }
        }
    }
    let mut usage: Vec<Usage> = usage
        .into_iter()
        .map(|(color, count)| Usage { color, count })
        .collect();
    usage.sort_by_key(|u| std::cmp::Reverse(u.count));
    let total_beads = usage.iter().map(|u| u.count).sum();
    // SVG 图纸
    let cell = 10.0;
    let mut svg = String::with_capacity(cells.len() * 40 + 256);
    svg.push_str(&format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{:.0}' height='{:.0}' viewBox='0 0 {:.0} {:.0}'>"#,
        columns as f64 * cell,
        rows as f64 * cell,
        columns as f64 * cell,
        rows as f64 * cell
    ));
    svg.push_str(r#"<rect width='100%' height='100%' fill='#ffffff'/>"#);
    for cell_data in &cells {
        let Some((_, r, g, b)) = PALETTE
            .iter()
            .find(|(name, _, _, _)| name == &cell_data.color)
        else {
            continue;
        };
        let x = cell_data.column as f64 * cell;
        let y = cell_data.row as f64 * cell;
        svg.push_str(&format!(
            r#"<circle cx='{:.1}' cy='{:.1}' r='{:.1}' fill='#{r:02x}{g:02x}{b:02x}' stroke='#d1d5db' stroke-width='0.4'/>"#,
            x + cell / 2.0,
            y + cell / 2.0,
            cell / 2.0 - 0.5
        ));
        if cell > 12.0 {
            svg.push_str(&format!(
                r#"<text x='{:.1}' y='{:.1}' font-size='{:.1}' text-anchor='middle' fill='#111827'>{}</text>"#,
                x + cell / 2.0,
                y + cell / 2.0 + 1.4,
                cell * 0.42,
                cell_data.symbol
            ));
        }
    }
    svg.push_str("</svg>");
    Ok(Output {
        columns,
        rows,
        cells,
        usage,
        total_beads,
        svg,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
