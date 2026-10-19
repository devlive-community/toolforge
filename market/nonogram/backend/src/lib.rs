//! 数织后端：题目生成（唯一性验证）、玩家棋盘校验、逻辑提示、题纸导出。
//! 进度与偏好由前端持久化（usePluginState → ToolForge SQLite）。

mod generate;
mod paper;
mod solver;

use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Nonogram {
    manifest: Manifest,
}

impl Default for Nonogram {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardArgs {
    clues: solver::Clues,
    /// 玩家棋盘：0=未定 1=黑 2=空，长度 size*size
    board: Vec<u8>,
}

fn validate_board(args: &BoardArgs) -> PluginResult<()> {
    solver::validate_clues(&args.clues)?;
    if args.board.len() != args.clues.size * args.clues.size {
        return Err(PluginError::new("puz.bad_board").with("size", args.board.len()));
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckOut {
    /// 全部行列都 done（无未定格且片段匹配）
    done: bool,
    row_done: Vec<bool>,
    col_done: Vec<bool>,
    /// 当前行/列与线索矛盾
    row_conflict: Vec<bool>,
    col_conflict: Vec<bool>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HintOut {
    found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    col: Option<usize>,
    /// true=填黑 false=标空
    #[serde(skip_serializing_if = "Option::is_none")]
    black: Option<bool>,
    /// 理由：0=行推理 1=列推理
    #[serde(skip_serializing_if = "Option::is_none")]
    via: Option<u8>,
}

impl ToolPlugin for Nonogram {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate::generate(parse_args(args)?)?),
            "check" => {
                let args: BoardArgs = parse_args(args)?;
                validate_board(&args)?;
                let (row_done, col_done, row_conflict, col_conflict) =
                    solver::check_lines(&args.clues, &args.board);
                to_value(CheckOut {
                    done: row_done.iter().all(|&x| x) && col_done.iter().all(|&x| x),
                    row_done,
                    col_done,
                    row_conflict,
                    col_conflict,
                })
            }
            "hint" => {
                let args: BoardArgs = parse_args(args)?;
                validate_board(&args)?;
                let out = match solver::find_hint(&args.clues, &args.board) {
                    Some(hint) => HintOut {
                        found: true,
                        row: Some(hint.row),
                        col: Some(hint.col),
                        black: Some(hint.black),
                        via: Some(hint.via),
                    },
                    None => HintOut {
                        found: false,
                        row: None,
                        col: None,
                        black: None,
                        via: None,
                    },
                };
                to_value(out)
            }
            "export_paper" => to_value(paper::export_paper(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Nonogram);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
