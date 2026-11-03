//! 反应与序列记忆挑战后端：等待时长与序列生成、反应成绩结算（有效轮中位数）、
//! 序列判定（首个错误位置）。时间戳由前端以单调时钟上报，不做跨进程测量。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct ReactionChallenge {
    manifest: Manifest,
}

impl Default for ReactionChallenge {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for ReactionChallenge {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "reaction_round" => to_value(reaction_round(parse_args(args)?)?),
            "reaction_score" => to_value(reaction_score(parse_args(args)?)?),
            "sequence" => to_value(sequence(parse_args(args)?)?),
            "sequence_judge" => to_value(sequence_judge(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(ReactionChallenge);

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) % n as u64) as usize
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundArgs {
    pub seed: u64,
    /// 随机等待下限（毫秒，默认 1000）
    #[serde(default)]
    pub min_ms: Option<u64>,
    /// 上限（默认 4000）
    #[serde(default)]
    pub max_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundOutput {
    /// 随机等待毫秒（前端在提示前等待）
    pub wait_ms: u64,
}

pub fn reaction_round(args: RoundArgs) -> PluginResult<RoundOutput> {
    let min = args.min_ms.unwrap_or(1000);
    let max = args.max_ms.unwrap_or(4000);
    if min > max || max - min > 10_000 || min < 200 {
        return Err(PluginError::new("reaction.bad_wait")
            .with("min", min)
            .with("max", max));
    }
    let mut rng = Rng::new(args.seed);
    Ok(RoundOutput {
        wait_ms: min + rng.below((max - min) as usize + 1) as u64,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreArgs {
    /// 各轮反应毫秒；抢跑轮传 null
    pub times_ms: Vec<Option<u64>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreOutput {
    pub valid_count: usize,
    pub false_starts: usize,
    /// 有效轮中位数（不足 3 个有效轮不记正式成绩）
    pub median_ms: Option<u64>,
    pub official: bool,
    /// 最快与最慢（展示）
    pub best_ms: Option<u64>,
    pub worst_ms: Option<u64>,
}

/// 固定 5 轮制的成绩结算
pub fn reaction_score(args: ScoreArgs) -> PluginResult<ScoreOutput> {
    if args.times_ms.is_empty() || args.times_ms.len() > 10 {
        return Err(
            PluginError::new("reaction.bad_rounds").with("rounds", args.times_ms.len() as u64)
        );
    }
    let false_starts = args.times_ms.iter().filter(|t| t.is_none()).count();
    let mut valid: Vec<u64> = args.times_ms.iter().filter_map(|t| *t).collect();
    if valid.iter().any(|t| *t == 0 || *t > 10_000) {
        return Err(PluginError::new("reaction.bad_time"));
    }
    valid.sort_unstable();
    let official = valid.len() >= 3;
    let median_ms = if valid.is_empty() {
        None
    } else {
        Some(valid[valid.len() / 2])
    };
    Ok(ScoreOutput {
        valid_count: valid.len(),
        false_starts,
        median_ms,
        official,
        best_ms: valid.first().copied(),
        worst_ms: valid.last().copied(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceArgs {
    pub seed: u64,
    /// 序列长度（1–32）
    pub length: usize,
    /// 格子数（4）
    #[serde(default)]
    pub pads: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceOutput {
    /// 格子序号序列（0 起）
    pub order: Vec<usize>,
}

pub fn sequence(args: SequenceArgs) -> PluginResult<SequenceOutput> {
    let pads = args.pads.unwrap_or(4);
    if !(2..=8).contains(&pads) {
        return Err(PluginError::new("reaction.bad_pads").with("pads", pads as u64));
    }
    if !(1..=32).contains(&args.length) {
        return Err(PluginError::new("reaction.bad_length").with("length", args.length as u64));
    }
    let mut rng = Rng::new(args.seed);
    let order = (0..args.length).map(|_| rng.below(pads)).collect();
    Ok(SequenceOutput { order })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeArgs {
    /// 本轮正确序列
    pub expected: Vec<usize>,
    /// 玩家输入（可长于 expected；超出部分不算错）
    pub entered: Vec<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeOutput {
    /// passed / failed
    pub status: &'static str,
    /// 首个错误位置（failed 时；0 起）
    pub error_at: Option<usize>,
    pub correct_len: usize,
}

pub fn sequence_judge(args: JudgeArgs) -> PluginResult<JudgeOutput> {
    if args.expected.is_empty() {
        return Err(PluginError::new("reaction.bad_length").with("length", 0));
    }
    let mut correct_len = 0usize;
    for (index, expected) in args.expected.iter().enumerate() {
        match args.entered.get(index) {
            Some(entered) if entered == expected => correct_len += 1,
            Some(entered) => {
                return Ok(JudgeOutput {
                    status: "failed",
                    error_at: Some(*entered),
                    correct_len,
                });
            }
            None => break,
        }
    }
    let status = if correct_len == args.expected.len() {
        "passed"
    } else {
        "failed"
    };
    Ok(JudgeOutput {
        status,
        error_at: None,
        correct_len,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
