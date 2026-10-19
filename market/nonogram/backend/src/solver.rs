//! 数织求解器：线索提取、单行/列约束传播（DP 枚举候选取交集）、
//! 全局求解（传播 + 受节点上限约束的分支搜索）用于唯一性验证。
//! 传播能把全部格子确定 ⇒ 唯一解；分支后两个方向都有解 ⇒ 多解。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 支持的棋盘规格
pub const SIZES: [usize; 3] = [5, 10, 15];
/// 分支搜索节点与深度上限（超限视为无法验证，不交付）
pub const MAX_NODES: u64 = 50_000;
pub const MAX_DEPTH: usize = 64;
/// 生成重试次数
pub const MAX_ATTEMPTS: usize = 40;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// 单格状态：0=未定 1=黑 2=空
pub const CELL_UNKNOWN: u8 = 0;
pub const CELL_BLACK: u8 = 1;
pub const CELL_WHITE: u8 = 2;

/// 从一行 0/1 提取连续黑段长度；全空行返回 [0]
pub fn extract_clue(line: &[bool]) -> Vec<u32> {
    let mut clue = Vec::new();
    let mut run = 0u32;
    for &filled in line {
        if filled {
            run += 1;
        } else if run > 0 {
            clue.push(run);
            run = 0;
        }
    }
    if run > 0 {
        clue.push(run);
    }
    if clue.is_empty() {
        clue.push(0);
    }
    clue
}

/// 整题线索：rows 长度 = 高，cols 长度 = 宽
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clues {
    pub size: usize,
    /// 每行线索（自上而下）
    pub rows: Vec<Vec<u32>>,
    /// 每列线索（自左而右）
    pub cols: Vec<Vec<u32>>,
}

pub fn extract_clues(solution: &[bool], size: usize) -> Clues {
    let mut rows = Vec::with_capacity(size);
    for r in 0..size {
        rows.push(extract_clue(&solution[r * size..(r + 1) * size]));
    }
    let mut cols = Vec::with_capacity(size);
    for c in 0..size {
        let line: Vec<bool> = (0..size).map(|r| solution[r * size + c]).collect();
        cols.push(extract_clue(&line));
    }
    Clues { size, rows, cols }
}

/// 单线求解：给定线索与当前已知状态，推导每个格子在所有合法放置中的取值集合。
/// 返回 None = 该线在当前状态下无解（矛盾）。
/// 输出 Vec<Option<bool>>：Some(true)=必黑 Some(false)=必空 None=未定。
fn solve_line(clue: &[u32], line: &[u8]) -> Option<Vec<Option<bool>>> {
    let n = line.len();
    let k = clue.len();
    let mut ctx = LineCtx {
        clue,
        line,
        n,
        visited: vec![false; (n + 1) * (k + 1)],
        black: vec![false; n],
        white: vec![false; n],
    };
    if !ctx.rec(0, 0) {
        return None;
    }
    Some(
        (0..n)
            .map(|i| {
                if ctx.black[i] && !ctx.white[i] {
                    Some(true)
                } else if ctx.white[i] && !ctx.black[i] {
                    Some(false)
                } else {
                    None
                }
            })
            .collect(),
    )
}

/// 单线求解上下文（收敛参数数量）
struct LineCtx<'a> {
    clue: &'a [u32],
    line: &'a [u8],
    n: usize,
    /// (pos, clue_index) 状态是否已展开为可行
    visited: Vec<bool>,
    /// 各格在合法放置中可为黑 / 可为白
    black: Vec<bool>,
    white: Vec<bool>,
}

impl LineCtx<'_> {
    fn rec(&mut self, pos: usize, ci: usize) -> bool {
        let index = pos * (self.clue.len() + 1) + ci;
        if self.visited[index] {
            // 只有可行状态才会被置 visited，再次到达直接复用
            return true;
        }
        // 可行性单独记忆化探测；不可行状态不置 visited
        if !self.feasible(pos, ci) {
            return false;
        }
        self.visited[index] = true;
        if pos == self.n {
            // feasible(n, ci) 成立当且仅当 ci == clue.len()
            return true;
        }
        let mut any = false;
        // 选项 A：pos 为白
        if self.line[pos] != CELL_BLACK && self.rec(pos + 1, ci) {
            self.white[pos] = true;
            any = true;
        }
        // 选项 B：pos 起放一段长度 run = clue[ci]
        if ci < self.clue.len() {
            let run = self.clue[ci] as usize;
            let end = pos + run;
            if end <= self.n
                && self.line[pos..end].iter().all(|&cell| cell != CELL_WHITE)
                && (end == self.n || self.line[end] != CELL_BLACK)
            {
                let next_pos = if end == self.n { self.n } else { end + 1 };
                if self.rec(next_pos, ci + 1) {
                    for cell in self.black.iter_mut().take(end).skip(pos) {
                        *cell = true;
                    }
                    if end < self.n {
                        self.white[end] = true;
                    }
                    any = true;
                }
            }
        }
        any
    }

    /// 可行性检查（不带格子标记，纯 DP 记忆化）
    fn feasible(&self, pos: usize, ci: usize) -> bool {
        let mut memo = vec![None; (self.n + 2) * (self.clue.len() + 2)];
        self.go(pos, ci, &mut memo)
    }

    fn go(&self, pos: usize, ci: usize, memo: &mut [Option<bool>]) -> bool {
        let index = pos * (self.clue.len() + 1) + ci;
        if let Some(value) = memo[index] {
            return value;
        }
        let value = if pos == self.n {
            ci == self.clue.len()
        } else {
            let mut ok = self.line[pos] != CELL_BLACK && self.go(pos + 1, ci, memo);
            if !ok && ci < self.clue.len() {
                let run = self.clue[ci] as usize;
                let end = pos + run;
                if end <= self.n
                    && self.line[pos..end].iter().all(|&cell| cell != CELL_WHITE)
                    && (end == self.n || self.line[end] != CELL_BLACK)
                {
                    let next_pos = if end == self.n { self.n } else { end + 1 };
                    ok = self.go(next_pos, ci + 1, memo);
                }
            }
            ok
        };
        memo[index] = Some(value);
        value
    }
}

/// 全局求解结果
#[derive(Debug, Clone, PartialEq)]
pub enum SolveResult {
    /// 传播 + 分支验证后确定全部格子（唯一解）
    Solved(Vec<bool>),
    /// 存在至少两个解
    Multiple,
    /// 线索无解
    Unsolvable,
    /// 节点/深度超限，无法验证
    Indeterminate,
}

struct Search {
    nodes: u64,
    max_nodes: u64,
    size: usize,
    clues: Clues,
}

impl Search {
    /// 对 grid 做一轮传播：返回 Ok(更新数)；矛盾 → Err
    fn propagate(&self, grid: &mut [u8]) -> Result<usize, ()> {
        let size = self.size;
        let mut updated = 0usize;
        loop {
            let mut changed = 0usize;
            for r in 0..size {
                let line: Vec<u8> = grid[r * size..(r + 1) * size].to_vec();
                let Some(derived) = solve_line(&self.clues.rows[r], &line) else {
                    return Err(());
                };
                for (c, value) in derived.into_iter().enumerate() {
                    if let Some(is_black) = value {
                        let cell = if is_black { CELL_BLACK } else { CELL_WHITE };
                        if grid[r * size + c] == CELL_UNKNOWN {
                            grid[r * size + c] = cell;
                            changed += 1;
                        } else if grid[r * size + c] != cell {
                            return Err(());
                        }
                    }
                }
            }
            for c in 0..size {
                let mut line = Vec::with_capacity(size);
                for r in 0..size {
                    line.push(grid[r * size + c]);
                }
                let Some(derived) = solve_line(&self.clues.cols[c], &line) else {
                    return Err(());
                };
                for (r, value) in derived.into_iter().enumerate() {
                    if let Some(is_black) = value {
                        let cell = if is_black { CELL_BLACK } else { CELL_WHITE };
                        if grid[r * size + c] == CELL_UNKNOWN {
                            grid[r * size + c] = cell;
                            changed += 1;
                        } else if grid[r * size + c] != cell {
                            return Err(());
                        }
                    }
                }
            }
            updated += changed;
            if changed == 0 {
                return Ok(updated);
            }
        }
    }

    fn solve(&mut self, grid: &mut [u8], depth: usize) -> SolveResult {
        if self.nodes >= self.max_nodes || depth > MAX_DEPTH {
            return SolveResult::Indeterminate;
        }
        self.nodes += 1;
        if self.propagate(grid).is_err() {
            return SolveResult::Unsolvable;
        }
        // 找第一个未定格
        let Some(index) = grid.iter().position(|&cell| cell == CELL_UNKNOWN) else {
            return SolveResult::Solved(grid.iter().map(|&cell| cell == CELL_BLACK).collect());
        };
        // 分支：黑 / 白 两个方向
        let mut branch_black = grid.to_vec();
        branch_black[index] = CELL_BLACK;
        let result_black = self.solve(&mut branch_black, depth + 1);
        if result_black == SolveResult::Indeterminate {
            return SolveResult::Indeterminate;
        }
        let mut branch_white = grid.to_vec();
        branch_white[index] = CELL_WHITE;
        let result_white = self.solve(&mut branch_white, depth + 1);
        if result_white == SolveResult::Indeterminate {
            return SolveResult::Indeterminate;
        }
        match (result_black, result_white) {
            (SolveResult::Unsolvable, SolveResult::Unsolvable) => SolveResult::Unsolvable,
            (SolveResult::Solved(board), SolveResult::Unsolvable)
            | (SolveResult::Unsolvable, SolveResult::Solved(board)) => SolveResult::Solved(board),
            _ => SolveResult::Multiple,
        }
    }
}

/// 求解入口：验证线索的唯一可解性
pub fn solve(clues: &Clues, max_nodes: u64) -> SolveResult {
    let size = clues.size;
    let mut search = Search {
        nodes: 0,
        max_nodes,
        size,
        clues: clues.clone(),
    };
    let mut grid = vec![CELL_UNKNOWN; size * size];
    search.solve(&mut grid, 0)
}

/// 校验线索结构：尺寸合法、行列线索数量一致
pub fn validate_clues(clues: &Clues) -> PluginResult<()> {
    if !SIZES.contains(&clues.size) {
        return Err(errp("puz.bad_size", "value", clues.size));
    }
    if clues.rows.len() != clues.size || clues.cols.len() != clues.size {
        return Err(errp("puz.bad_clues", "size", clues.size));
    }
    Ok(())
}

/// 检查玩家棋盘：行/列黑格片段是否与线索一致。
/// 返回 (row_done, col_done, row_conflict, col_conflict)。
/// done 要求该线无未定格且片段完全匹配；conflict 表示当前黑格片段已与线索矛盾。
pub fn check_lines(clues: &Clues, board: &[u8]) -> (Vec<bool>, Vec<bool>, Vec<bool>, Vec<bool>) {
    let size = clues.size;
    let row_done = (0..size)
        .map(|r| line_done(&clues.rows[r], &board[r * size..(r + 1) * size]))
        .collect();
    let mut col_done = Vec::with_capacity(size);
    let mut col_conflict = Vec::with_capacity(size);
    for c in 0..size {
        let line: Vec<u8> = (0..size).map(|r| board[r * size + c]).collect();
        col_done.push(line_done(&clues.cols[c], &line));
        col_conflict.push(line_conflict(&clues.cols[c], &line));
    }
    let row_conflict = (0..size)
        .map(|r| line_conflict(&clues.rows[r], &board[r * size..(r + 1) * size]))
        .collect();
    (row_done, col_done, row_conflict, col_conflict)
}

/// 无未定格且黑格片段与线索一致
fn line_done(clue: &[u32], line: &[u8]) -> bool {
    if line.contains(&CELL_UNKNOWN) {
        return false;
    }
    runs(line) == clue
}

/// 当前黑格片段是否已经与线索矛盾（未定格按可黑可白处理，用单线可解性判断）
fn line_conflict(clue: &[u32], line: &[u8]) -> bool {
    solve_line(clue, line).is_none()
}

/// 黑格片段长度（全空 → [0]）
fn runs(line: &[u8]) -> Vec<u32> {
    let mut clue = Vec::new();
    let mut run = 0u32;
    for &cell in line {
        if cell == CELL_BLACK {
            run += 1;
        } else if run > 0 {
            clue.push(run);
            run = 0;
        }
    }
    if run > 0 {
        clue.push(run);
    }
    if clue.is_empty() {
        clue.push(0);
    }
    clue
}

/// 提示：找一格可由单线推理确定的位置（行/列候选重合），并说明理由。
/// 找不到时如实返回（需要分支推理，首版不提供）。
pub struct Hint {
    pub row: usize,
    pub col: usize,
    /// true=填黑 false=标空
    pub black: bool,
    /// 理由线索来源：0=行 1=列（首个可解释的线）
    pub via: u8,
}

pub fn find_hint(clues: &Clues, board: &[u8]) -> Option<Hint> {
    let size = clues.size;
    // 先行后列逐线扫描，返回第一个新确定格
    for r in 0..size {
        let line: Vec<u8> = board[r * size..(r + 1) * size].to_vec();
        if let Some(derived) = solve_line(&clues.rows[r], &line) {
            for (c, value) in derived.into_iter().enumerate() {
                if let Some(is_black) = value
                    && board[r * size + c] == CELL_UNKNOWN
                {
                    return Some(Hint {
                        row: r,
                        col: c,
                        black: is_black,
                        via: 0,
                    });
                }
            }
        }
    }
    for c in 0..size {
        let mut line = Vec::with_capacity(size);
        for r in 0..size {
            line.push(board[r * size + c]);
        }
        if let Some(derived) = solve_line(&clues.cols[c], &line) {
            for (r, value) in derived.into_iter().enumerate() {
                if let Some(is_black) = value
                    && board[r * size + c] == CELL_UNKNOWN
                {
                    return Some(Hint {
                        row: r,
                        col: c,
                        black: is_black,
                        via: 1,
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "solver_test.rs"]
mod tests;
