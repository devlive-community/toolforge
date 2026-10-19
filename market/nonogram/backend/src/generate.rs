//! 题目生成：随机原创图案 → 提取线索 → 独立求解验证唯一性；
//! 不唯一/矛盾/超限的题目直接丢弃重试，超限失败时明确报错，不交付弱题。

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tf_plugin_api::{PluginError, PluginResult};

use crate::solver::{self, Clues, MAX_ATTEMPTS, MAX_NODES, SIZES, SolveResult};

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

/// xorshift64：WASM 无 rand，用纳秒时间做种子的确定性伪随机
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // 避免种子为 0 时恒定输出
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    /// 棋盘规格：5 / 10 / 15
    pub size: usize,
    /// 可选种子（复现测试）；缺省用纳秒时间
    pub seed: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Puzzle {
    pub size: usize,
    pub clues: Clues,
    /// 唯一解（行优先布尔）
    pub solution: Vec<bool>,
    /// 尝试次数（诊断）
    pub attempts: usize,
}

/// 生成一道唯一解题目；验证失败的图案一律丢弃重试
pub fn generate(args: GenerateArgs) -> PluginResult<Puzzle> {
    if !SIZES.contains(&args.size) {
        return Err(errp("puz.bad_size", "value", args.size));
    }
    let size = args.size;
    let seed = args.seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15)
    });
    let mut rng = Rng::new(seed);
    // 填充概率略低于 0.5 时更易产生唯一解的紧凑图案
    let black_percent: [usize; 3] = [48, 50, 52];
    let density = black_percent[rng.below(3)];
    for attempt in 1..=MAX_ATTEMPTS {
        let mut solution = Vec::with_capacity(size * size);
        for _ in 0..size * size {
            solution.push(rng.below(100) < density);
        }
        let clues = solver::extract_clues(&solution, size);
        match solver::solve(&clues, MAX_NODES) {
            SolveResult::Solved(board) => {
                // 唯一解必须与源图案一致（线索由它提取，理论必然，双保险校验）
                if board == solution {
                    return Ok(Puzzle {
                        size,
                        clues,
                        solution,
                        attempts: attempt,
                    });
                }
            }
            // 多解 / 无解 / 无法验证：全部重试
            _ => continue,
        }
    }
    Err(PluginError::new("puz.generation_failed").with("attempts", MAX_ATTEMPTS))
}

#[cfg(test)]
#[path = "generate_test.rs"]
mod tests;
