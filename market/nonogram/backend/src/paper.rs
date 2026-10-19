//! 题纸导出：无答案题纸 + 独立答案页，输出单个 HTML（@media print 分页），
//! 用户浏览器打印即可得到纸质题。线索用表格布局保证不裁切。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::solver::Clues;

fn err(code: &str) -> PluginError {
    PluginError::new(code)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperArgs {
    pub clues: Clues,
    /// 答案（行优先布尔），导出答案页必需
    pub solution: Option<Vec<bool>>,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperOut {
    pub html: String,
}

fn clue_list_html(clue: &[u32]) -> String {
    if clue.len() == 1 && clue[0] == 0 {
        return "<span class=\"zero\">0</span>".to_owned();
    }
    clue.iter()
        .map(|n| format!("<span>{n}</span>"))
        .collect::<Vec<_>>()
        .join("")
}

pub fn export_paper(args: PaperArgs) -> PluginResult<PaperOut> {
    let size = args.clues.size;
    crate::solver::validate_clues(&args.clues)?;
    let title = if args.title.trim().is_empty() {
        format!("Nonogram {size}×{size}")
    } else {
        args.title.trim().to_owned()
    };

    // 题纸：顶部列线索（每列自下而上排列靠边），左侧行线索，空网格
    let mut col_heads = String::new();
    for clue in &args.clues.cols {
        col_heads.push_str("<td><div class=\"col-clue\">");
        // 列线索从上到下排列，底部对齐
        for number in clue.iter().rev() {
            col_heads.push_str(&format!(
                "<span>{}</span>",
                if *number == 0 {
                    "0".to_owned()
                } else {
                    number.to_string()
                }
            ));
        }
        col_heads.push_str("</div></td>");
    }
    let mut body = String::new();
    for r in 0..size {
        body.push_str("<tr>");
        body.push_str(&format!(
            "<td><div class=\"row-clue\">{}</div></td>",
            clue_list_html(&args.clues.rows[r])
        ));
        for _ in 0..size {
            body.push_str("<td><div class=\"cell\"></div></td>");
        }
        body.push_str("</tr>");
    }

    // 答案页
    let Some(solution_bools) = args.solution else {
        return Err(err("puz.solution_required"));
    };
    if solution_bools.len() != size * size {
        return Err(err("puz.solution_size"));
    }
    let mut solution_grid = String::from("<table class=\"grid\">");
    for r in 0..size {
        solution_grid.push_str("<tr><td></td>");
        for c in 0..size {
            let cell = solution_bools[r * size + c];
            solution_grid.push_str(if cell {
                "<td><div class=\"cell black\"></div></td>"
            } else {
                "<td><div class=\"cell white\"></div></td>"
            });
        }
        solution_grid.push_str("</tr>");
    }
    solution_grid.push_str("</table>");

    let html = format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>{title}</title>
<style>
  body {{ font-family: sans-serif; color: #111; margin: 24px; }}
  .page {{ page-break-after: always; }}
  h1 {{ font-size: 18px; }}
  table {{ border-collapse: collapse; }}
  td {{ border: none; padding: 0; }}
  .grid td {{ border: 1px solid #999; }}
  .cell {{ width: 26px; height: 26px; }}
  .cell.black {{ background: #111; }}
  .cell.white {{ background: #fff; }}
  .col-clue {{ display: flex; flex-direction: column; justify-content: flex-end; align-items: center; height: {head_h}px; }}
  .row-clue {{ display: flex; justify-content: flex-end; align-items: center; gap: 4px; height: 26px; padding-right: 6px; }}
  .col-clue span {{ font-size: 12px; line-height: 1.1; }}
  .row-clue span {{ font-size: 12px; }}
  .zero {{ color: #bbb; }}
  .clue-head {{ background: transparent; }}
  @media print {{
    .no-print {{ display: none; }}
    body {{ margin: 0; }}
  }}
</style></head>
<body>
<div class="page">
  <h1>{title}</h1>
  <table class="grid">
    <tr><td></td>{col_heads}</tr>
    {body}
  </table>
</div>
<div class="page">
  <h1>{title} — {answer_word}</h1>
  {solution_grid}
</div>
</body></html>
"#,
        title = title,
        head_h = (size * 14).max(60),
        answer_word = "答案 / Solution",
    );
    Ok(PaperOut { html })
}
