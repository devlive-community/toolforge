//! 命令行 / Agent 用的扁平入口：按行列数与种子生成迷宫，输出字符画与可在界面打开的迷宫码。
//!
//! 生成、出入口与答案路线都复用界面同一套算法（maze::generate / route）；同一组参数与种子
//! 总是得到同一张迷宫。字符画里 S 为起点、E 为终点，answer 时用 · 画出答案路线。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginResult, TaskContext};

use crate::maze::{ALGORITHM, Maze, Port, Side, Spec, check, generate};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawArgs {
    #[serde(default = "ten")]
    pub rows: usize,
    #[serde(default = "ten")]
    pub cols: usize,
    /// 种子；缺省按当前时间生成
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default = "left")]
    pub entrance: Side,
    #[serde(default = "right")]
    pub exit: Side,
    #[serde(default)]
    pub answer: bool,
}

fn ten() -> usize {
    10
}
fn left() -> Side {
    Side::Left
}
fn right() -> Side {
    Side::Right
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrawOut {
    pub text: String,
    /// 迷宫码：在界面里导入即可得到同一张迷宫
    pub code: String,
    pub id: String,
    pub seed: String,
    /// 答案路线经过的格数
    pub route_cells: usize,
}

/// 出入口放在所在边的起点或终点：入口在边的第一格，出口在边的最后一格
fn port(side: Side, rows: usize, cols: usize, last: bool) -> Port {
    let len = match side {
        Side::Top | Side::Bottom => cols,
        Side::Left | Side::Right => rows,
    };
    Port {
        side,
        index: if last { len.max(1) } else { 1 },
    }
}

pub fn ascii(maze: &Maze, route: &[usize]) -> String {
    let (rows, cols) = (maze.spec.rows, maze.spec.cols);
    let open = |cell: usize, side: Side| maze.passages[cell] & side.bit() != 0;
    let mut on_route = vec![false; rows * cols];
    for &c in route {
        on_route[c] = true;
    }
    let mut out = String::new();
    for r in 0..rows {
        // 上边
        for c in 0..cols {
            out.push('+');
            out.push_str(if open(r * cols + c, Side::Top) {
                "   "
            } else {
                "---"
            });
        }
        out.push_str("+\n");
        // 格子与左右墙
        for c in 0..cols {
            let cell = r * cols + c;
            out.push(if open(cell, Side::Left) { ' ' } else { '|' });
            let mark = if cell == maze.start {
                " S "
            } else if cell == maze.exit {
                " E "
            } else if on_route[cell] {
                " · "
            } else {
                "   "
            };
            out.push_str(mark);
        }
        out.push(if open(r * cols + cols - 1, Side::Right) {
            ' '
        } else {
            '|'
        });
        out.push('\n');
    }
    for c in 0..cols {
        out.push('+');
        out.push_str(if open((rows - 1) * cols + c, Side::Bottom) {
            "   "
        } else {
            "---"
        });
    }
    out.push_str("+\n");
    out
}

pub fn draw(args: DrawArgs, ctx: &dyn TaskContext) -> PluginResult<DrawOut> {
    let seed = args
        .seed
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("cli-{}", jiff::Timestamp::now().as_millisecond()));
    let spec = Spec {
        algorithm: ALGORITHM.into(),
        rows: args.rows,
        cols: args.cols,
        entrance: port(args.entrance, args.rows, args.cols, false),
        exit: port(args.exit, args.rows, args.cols, true),
        seed: seed.clone(),
    };
    let code = spec.code();
    let id = spec.id();
    ctx.stage("generate");
    let maze = generate(spec, ctx)?;
    check(ctx)?;
    let route = maze.route(maze.start, ctx)?;
    let text = ascii(&maze, if args.answer { &route } else { &[] });
    Ok(DrawOut {
        text,
        code,
        id,
        seed,
        route_cells: route.len(),
    })
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
