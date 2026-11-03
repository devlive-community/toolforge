//! 汉字字帖生成器后端：把文本按田字格/米字格/方格布局为分页字帖数据，
//! 附 SVG 生成（网格 + 示范字 + 描红字，字体由查看端以系统字体渲染）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct HanziCopybook {
    manifest: Manifest,
}

impl Default for HanziCopybook {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for HanziCopybook {
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

tf_plugin_guest::export_plugin!(HanziCopybook);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub text: String,
    /// grid / tian / mi
    #[serde(default)]
    pub grid_type: Option<String>,
    /// 每行字数（4–16）
    #[serde(default)]
    pub per_row: Option<usize>,
    /// 每页行数（2–20）
    #[serde(default)]
    pub rows_per_page: Option<usize>,
    /// 每字重复次数（1–10）
    #[serde(default)]
    pub repeat: Option<usize>,
    /// empty / trace：empty 每行只有一个示范字，其余空格；trace 示范 + 描红交替
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub ch: char,
    /// normal（示范）/ trace（描红）/ empty（空格）
    pub kind: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub number: u64,
    pub rows: Vec<Vec<Cell>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutOutput {
    pub grid_type: String,
    pub per_row: usize,
    pub rows_per_page: usize,
    pub pages: Vec<Page>,
    /// 无法渲染的控制字符（被跳过）
    pub skipped: usize,
}

const SKIP_CHARS: &[char] = &['\n', '\r', '\t', '\u{0}', '\u{feff}'];

fn is_printable(ch: char) -> bool {
    !SKIP_CHARS.contains(&ch) && !ch.is_control()
}

pub fn layout(args: Args) -> PluginResult<LayoutOutput> {
    let grid_type = args.grid_type.unwrap_or_else(|| "tian".into());
    if !matches!(grid_type.as_str(), "grid" | "tian" | "mi") {
        return Err(PluginError::new("copybook.bad_grid").with("gridType", grid_type));
    }
    let per_row = args.per_row.unwrap_or(10);
    let rows_per_page = args.rows_per_page.unwrap_or(8);
    let repeat = args.repeat.unwrap_or(4);
    if !(4..=16).contains(&per_row) {
        return Err(PluginError::new("copybook.bad_per_row").with("perRow", per_row as u64));
    }
    if !(2..=20).contains(&rows_per_page) {
        return Err(PluginError::new("copybook.bad_rows").with("rowsPerPage", rows_per_page as u64));
    }
    if !(1..=10).contains(&repeat) {
        return Err(PluginError::new("copybook.bad_repeat").with("repeat", repeat as u64));
    }
    let mode = args.mode.unwrap_or_else(|| "trace".into());
    if !matches!(mode.as_str(), "empty" | "trace") {
        return Err(PluginError::new("copybook.bad_mode").with("mode", mode));
    }
    let mut skipped = 0usize;
    let mut cells: Vec<Cell> = Vec::new();
    for ch in args.text.chars() {
        if !is_printable(ch) {
            skipped += 1;
            continue;
        }
        // 空格按一个空单元处理
        let (kind, show) = if ch == ' ' {
            ("empty", None)
        } else {
            (if mode == "trace" { "trace" } else { "normal" }, Some(ch))
        };
        if mode == "trace" && show.is_some() {
            // 示范 + 描红：第一个 normal，其余 repeat-1 个 trace
            cells.push(Cell { ch, kind: "normal" });
            for _ in 1..repeat {
                cells.push(Cell { ch, kind: "trace" });
            }
        } else {
            for _ in 0..repeat {
                cells.push(Cell {
                    ch: show.unwrap_or(ch),
                    kind,
                });
            }
        }
    }
    if cells.is_empty() {
        return Err(PluginError::new("copybook.empty_text"));
    }
    // 按行补齐空格
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for chunk in cells.chunks(per_row) {
        let mut row: Vec<Cell> = chunk.to_vec();
        while row.len() < per_row {
            row.push(Cell {
                ch: ' ',
                kind: "empty",
            });
        }
        rows.push(row);
    }
    let mut pages = Vec::new();
    for (page_index, chunk) in rows.chunks(rows_per_page).enumerate() {
        pages.push(Page {
            number: (page_index + 1) as u64,
            rows: chunk.to_vec(),
        });
    }
    if pages.len() > 50 {
        return Err(PluginError::new("copybook.too_many_pages")
            .with("pages", pages.len() as u64)
            .with("max", 50));
    }
    Ok(LayoutOutput {
        grid_type,
        per_row,
        rows_per_page,
        pages,
        skipped,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
