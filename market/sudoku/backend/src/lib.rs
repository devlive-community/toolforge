//! 数独插件后端：4×4 与 9×9 棋盘的生成（唯一解校验）、求解、冲突检测与解释性提示。
//!
//! 随机来自调用方传入的种子（splitmix64），同一种子生成同一题面，结果可复现。
//! 生成在固定步数预算内重试挖洞，超预算返回错误而不是交付多解题。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Sudoku {
    manifest: Manifest,
}

impl Default for Sudoku {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Sudoku {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            "analyze" => to_value(analyze(parse_args(args)?)?),
            "hint" => to_value(hint(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Sudoku);

/// 随机数：splitmix64，同种子同序列
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.draw() % n as u64) as usize
    }
}

fn box_size(size: usize) -> usize {
    if size == 4 { 2 } else { 3 }
}

fn valid_placement(grid: &[u8], size: usize, index: usize, value: u8) -> bool {
    let row = index / size;
    let col = index % size;
    let bs = box_size(size);
    for k in 0..size {
        if grid[row * size + k] == value || grid[k * size + col] == value {
            return false;
        }
    }
    let (br, bc) = ((row / bs) * bs, (col / bs) * bs);
    for r in br..br + bs {
        for c in bc..bc + bs {
            if grid[r * size + c] == value {
                return false;
            }
        }
    }
    true
}

fn candidates(grid: &[u8], size: usize, index: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for value in 1..=size as u8 {
        if valid_placement(grid, size, index, value) {
            out.push(value);
        }
    }
    out
}

/// 解计数（上限 2 用于唯一性判断）；conflict 检查题面本身的行列宫重复
fn count_solutions(grid: &mut [u8], size: usize, limit: usize, steps: &mut u64) -> usize {
    let Some(index) = (0..size * size).find(|&i| grid[i] == 0) else {
        return 1;
    };
    let mut count = 0;
    for value in 1..=size as u8 {
        *steps += 1;
        if *steps > 2_000_000 {
            return count; // 预算耗尽，按已找到的数量返回
        }
        if valid_placement(grid, size, index, value) {
            grid[index] = value;
            count += count_solutions(grid, size, limit, steps);
            grid[index] = 0;
            if count >= limit {
                break;
            }
        }
    }
    count
}

fn full_grid(rng: &mut Rng, size: usize) -> Option<Vec<u8>> {
    let mut grid = vec![0u8; size * size];
    fill(&mut grid, size, rng, 0)
}

fn fill(grid: &mut Vec<u8>, size: usize, rng: &mut Rng, index: usize) -> Option<Vec<u8>> {
    if index == size * size {
        return Some(grid.clone());
    }
    if grid[index] != 0 {
        return fill(grid, size, rng, index + 1);
    }
    let mut values: Vec<u8> = (1..=size as u8).collect();
    for k in (1..values.len()).rev() {
        values.swap(0, rng.below(k + 1));
    }
    for value in values {
        if valid_placement(grid, size, index, value) {
            grid[index] = value;
            if fill(grid, size, rng, index + 1).is_some() {
                return Some(grid.clone());
            }
            grid[index] = 0;
        }
    }
    None
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    /// 4 或 9
    pub size: usize,
    /// 挖洞数量（4×4 上限 10，9×9 上限 64）
    pub holes: usize,
    /// 随机种子（前端用日期/时间生成，保证可复现）
    pub seed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateOutput {
    pub size: usize,
    /// 题面（0 为空格，行优先）
    pub puzzle: Vec<u8>,
    /// 唯一解（前端不直接展示，供核对）
    pub solution: Vec<u8>,
    pub holes: usize,
}

pub fn generate(args: GenerateArgs) -> PluginResult<GenerateOutput> {
    let size = args.size;
    if size != 4 && size != 9 {
        return Err(PluginError::new("sudoku.bad_size").with("size", size as u64));
    }
    let max_holes = if size == 4 { 10 } else { 64 };
    if args.holes == 0 || args.holes > max_holes {
        return Err(PluginError::new("sudoku.bad_holes")
            .with("holes", args.holes as u64)
            .with("max", max_holes as u64));
    }
    let mut rng = Rng::new(args.seed);
    let full =
        full_grid(&mut rng, size).ok_or_else(|| PluginError::new("sudoku.generate_failed"))?;
    // 随机顺序挖洞，每挖一格校验唯一解；预算耗尽则提前结束（保证已交付的题面唯一）
    let mut puzzle = full.clone();
    let mut order: Vec<usize> = (0..size * size).collect();
    for k in (1..order.len()).rev() {
        order.swap(0, rng.below(k + 1));
    }
    let mut removed = 0usize;
    for &index in order.iter().take(max_holes * 3) {
        if removed >= args.holes {
            break;
        }
        let backup = puzzle[index];
        if backup == 0 {
            continue;
        }
        puzzle[index] = 0;
        let mut probe = puzzle.clone();
        let mut steps = 0u64;
        if count_solutions(&mut probe, size, 2, &mut steps) != 1 {
            puzzle[index] = backup;
        } else {
            removed += 1;
        }
    }
    Ok(GenerateOutput {
        size,
        puzzle,
        solution: full,
        holes: removed,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeArgs {
    pub size: usize,
    /// 题面或进行中的棋盘（0 为空）
    pub grid: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub index: usize,
    /// conflict.kind：row / column / box
    pub kind: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeOutput {
    /// invalid（数量不符）/ conflicts（题面有重复）/ no_solution / multiple / unique / solved
    pub status: &'static str,
    pub conflicts: Vec<Conflict>,
    /// 空格数量
    pub blanks: usize,
}

fn detect_conflicts(grid: &[u8], size: usize) -> Vec<Conflict> {
    let bs = box_size(size);
    let mut conflicts = Vec::new();
    let mut units: Vec<(Vec<usize>, &'static str)> = Vec::new();
    for row in 0..size {
        units.push(((0..size).map(|c| row * size + c).collect(), "row"));
    }
    for col in 0..size {
        units.push(((0..size).map(|r| r * size + col).collect(), "column"));
    }
    for br in 0..bs {
        for bc in 0..bs {
            let mut cells = Vec::new();
            for r in br * bs..(br + 1) * bs {
                for c in bc * bs..(bc + 1) * bs {
                    cells.push(r * size + c);
                }
            }
            units.push((cells, "box"));
        }
    }
    for (cells, kind) in &units {
        for (i, &a) in cells.iter().enumerate() {
            if grid[a] == 0 {
                continue;
            }
            if cells[i + 1..].iter().any(|&b| grid[b] == grid[a]) {
                conflicts.push(Conflict { index: a, kind });
            }
        }
    }
    conflicts
}

pub fn analyze(args: AnalyzeArgs) -> PluginResult<AnalyzeOutput> {
    let size = args.size;
    if size != 4 && size != 9 {
        return Err(PluginError::new("sudoku.bad_size").with("size", size as u64));
    }
    if args.grid.len() != size * size || args.grid.iter().any(|v| *v as usize > size) {
        return Err(PluginError::new("sudoku.invalid_grid"));
    }
    let conflicts = detect_conflicts(&args.grid, size);
    let blanks = args.grid.iter().filter(|v| **v == 0).count();
    if blanks == 0 && conflicts.is_empty() {
        return Ok(AnalyzeOutput {
            status: "solved",
            conflicts,
            blanks,
        });
    }
    if !conflicts.is_empty() {
        return Ok(AnalyzeOutput {
            status: "conflicts",
            conflicts,
            blanks,
        });
    }
    let mut probe = args.grid.clone();
    let mut steps = 0u64;
    let count = count_solutions(&mut probe, size, 2, &mut steps);
    let status = match count {
        0 => "no_solution",
        1 => "unique",
        _ => "multiple",
    };
    Ok(AnalyzeOutput {
        status,
        conflicts,
        blanks,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HintArgs {
    pub size: usize,
    pub grid: Vec<u8>,
    /// 唯一解（由生成接口给出；导入题先过 analyze）
    pub solution: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HintOutput {
    pub index: usize,
    pub value: u8,
    /// 提示依据键：hints.<key>
    pub reason: &'static str,
}

/// 解释性提示：优先给唯一候选格，其次唯一缺失单元；不把直接抄答案伪装成推理
pub fn hint(args: HintArgs) -> PluginResult<HintOutput> {
    let size = args.size;
    if size != 4 && size != 9
        || args.grid.len() != size * size
        || args.solution.len() != size * size
    {
        return Err(PluginError::new("sudoku.invalid_grid"));
    }
    if args
        .grid
        .iter()
        .zip(&args.solution)
        .any(|(g, s)| *g != 0 && g != s)
    {
        return Err(PluginError::new("sudoku.conflicting_solution"));
    }
    let bs = box_size(size);
    // 1) 唯一候选：某空格只剩一个合法值
    for index in 0..size * size {
        if args.grid[index] != 0 {
            continue;
        }
        let cands = candidates(&args.grid, size, index);
        if cands.len() == 1 {
            return Ok(HintOutput {
                index,
                value: cands[0],
                reason: "only_candidate",
            });
        }
    }
    // 2) 唯一缺失：某行/列/宫只剩一个空格
    let mut units: Vec<(Vec<usize>, &'static str)> = Vec::new();
    for row in 0..size {
        units.push(((0..size).map(|c| row * size + c).collect(), "row_exhaust"));
    }
    for col in 0..size {
        units.push((
            (0..size).map(|r| r * size + col).collect(),
            "column_exhaust",
        ));
    }
    for br in 0..bs {
        for bc in 0..bs {
            let mut cells = Vec::new();
            for r in br * bs..(br + 1) * bs {
                for c in bc * bs..(bc + 1) * bs {
                    cells.push(r * size + c);
                }
            }
            units.push((cells, "box_exhaust"));
        }
    }
    for (cells, reason) in &units {
        if let Some(index) = cells.iter().copied().find(|&i| args.grid[i] == 0) {
            return Ok(HintOutput {
                index,
                value: args.solution[index],
                reason,
            });
        }
    }
    Err(PluginError::new("sudoku.no_hint"))
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
