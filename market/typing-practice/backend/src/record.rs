//! 一局结束后的纪录与关卡结算：是否过关、是否刷新个人最好、纪录数量上限。
//!
//! - 纪录桶由前端给出（来源 + 语言 + 模式 + 内容 + 内容指纹），不同桶互不比较，
//!   因此内置与自定义内容、不同版本的文本各自独立。
//! - 粘贴过的局只作练习：既不写纪录也不解锁关卡，避免用粘贴答案闯关。
//! - 没有有效速度（零时长 / 无正确字符）的局不写纪录。
//! - 更好 = CPM 更高；CPM 相同时准确率更高。
//! - 纪录最多保留 `MAX_BESTS` 条，超出时淘汰最早的一条。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 个人纪录条数上限（自定义文本每个版本一桶，需要防止无限增长）
pub const MAX_BESTS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Best {
    pub bucket: String,
    pub cpm: Option<f64>,
    pub wpm: Option<f64>,
    pub accuracy: Option<f64>,
    pub active_ms: u64,
    /// 完成时间（ISO 8601，由前端给出；同格式字符串可直接比较先后）
    pub at: String,
}

/// 一局的结果（来自 score 的最后一次输出 + 用时）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub cpm: Option<f64>,
    pub wpm: Option<f64>,
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub active_ms: u64,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub pasted: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub bests: BTreeMap<String, Best>,
    /// 本局是否写入正式纪录（未粘贴且有有效速度）
    pub recorded: bool,
    /// 是否刷新了该桶的个人最好
    pub new_best: bool,
    /// 是否解锁当前关卡（仅内置关卡、未粘贴且过关）
    pub cleared: bool,
}

fn better(new: &Best, old: &Best) -> bool {
    let (a, b) = (new.cpm.unwrap_or(0.0), old.cpm.unwrap_or(0.0));
    if a != b {
        return a > b;
    }
    new.accuracy.unwrap_or(0.0) > old.accuracy.unwrap_or(0.0)
}

/// 结算一局。`stage` 为真表示这是内置关卡（参与解锁）。
pub fn settle(
    mut bests: BTreeMap<String, Best>,
    bucket: &str,
    stage: bool,
    round: &Round,
    at: &str,
) -> Outcome {
    let recorded = !round.pasted && round.cpm.is_some();
    let cleared = stage && !round.pasted && round.passed;
    let mut new_best = false;
    if recorded {
        let candidate = Best {
            bucket: bucket.to_string(),
            cpm: round.cpm,
            wpm: round.wpm,
            accuracy: round.accuracy,
            active_ms: round.active_ms,
            at: at.to_string(),
        };
        new_best = bests.get(bucket).is_none_or(|old| better(&candidate, old));
        if new_best {
            bests.insert(bucket.to_string(), candidate);
        }
    }
    while bests.len() > MAX_BESTS {
        let oldest = bests
            .iter()
            .filter(|(key, _)| key.as_str() != bucket)
            .min_by(|a, b| a.1.at.cmp(&b.1.at))
            .map(|(key, _)| key.clone());
        match oldest {
            Some(key) => bests.remove(&key),
            None => break,
        };
    }
    Outcome {
        bests,
        recorded,
        new_best,
        cleared,
    }
}

#[cfg(test)]
#[path = "record_test.rs"]
mod tests;
