//! 打字成绩计算：逐字对照、错误尝试计数、准确率与速度。
//!
//! 口径（写在这里，界面与记忆文档引用同一份说明）：
//! - 按 Unicode 标量值（char）逐字对照，不按字节、不按字形簇拆分。
//! - 每一次提交的字符都算一次「尝试」；退格只清除画面上的字符，不删除已经发生的错误尝试。
//! - 准确率 = 正确尝试 / 全部尝试；全部尝试为 0 时准确率为 None（不显示）。
//! - CPM（每分钟正确字符数）= 正确尝试 / 分钟；WPM = CPM / 5，仅英文模式展示。
//! - 用时为 0 或没有任何输入时，速度为 None，避免产生无效或无穷大的速度。

use serde::{Deserialize, Serialize};

/// 一次提交的输入：练习区当前的完整文本，以及本次新增的字符数。
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// 练习区当前全部文本（退格之后的结果）
    pub typed: String,
    /// 本次相对上一次快照新增的字符数（多字提交与粘贴都按真实增量计）
    pub added: u32,
    /// 本次新增字符中，写入位置与目标不一致的个数（前端在提交时即可确定，错误尝试只增不减）
    pub wrong_added: u32,
    /// 是否由粘贴或导入动作产生（前端检测到 paste / drop 事件时置 true）
    #[serde(default)]
    pub pasted: bool,
}

/// 一局的累计状态。由 Rust 维护，前端只保存并回传。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    /// 累计提交的字符尝试次数（含后来被退格清除的）
    pub attempts: u32,
    /// 累计错误尝试次数（含后来被纠正的）
    pub wrong_attempts: u32,
    /// 本局是否出现过粘贴；出现过则本局标记为练习模式，不计正式纪录
    pub pasted: bool,
    /// 有效输入时长（毫秒），由前端根据暂停与恢复累计
    pub active_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub correct: u32,
    pub wrong_now: u32,
    pub total: u32,
    pub progress: f64,
    pub accuracy: Option<f64>,
    pub cpm: Option<f64>,
    pub wpm: Option<f64>,
    pub completed: bool,
    pub pasted: bool,
}

/// 逐字对照 target 与 typed，返回画面上仍然正确的字符数与仍错误的字符数。
fn compare(target: &str, typed: &str) -> (u32, u32) {
    let mut correct = 0u32;
    let mut wrong = 0u32;
    let mut t = target.chars();
    for typed_char in typed.chars() {
        match t.next() {
            Some(expected) if expected == typed_char => correct += 1,
            _ => wrong += 1,
        }
    }
    (correct, wrong)
}

/// 应用一次提交，返回更新后的 Session 与当前成绩。
///
/// `total` 是目标文本的字符数，`elapsed_ms` 是本局有效输入时长。
pub fn apply(
    target: &str,
    session: &Session,
    snapshot: &Snapshot,
    elapsed_ms: u64,
) -> (Session, Score) {
    let mut next = session.clone();
    next.attempts = next.attempts.saturating_add(snapshot.added);
    next.pasted = next.pasted || snapshot.pasted;
    next.active_ms = elapsed_ms;

    let (correct, wrong_now) = compare(target, &snapshot.typed);
    let total = u32::try_from(target.chars().count()).unwrap_or(u32::MAX);
    // 错误尝试只增不减：本次新增中不符合目标位置的字符计入累计，之后退格纠正也不减少
    next.wrong_attempts = next
        .wrong_attempts
        .saturating_add(snapshot.wrong_added.min(snapshot.added));

    let score = build_score(total, correct, wrong_now, &next);
    (next, score)
}

fn build_score(total: u32, correct: u32, wrong_now: u32, session: &Session) -> Score {
    let attempts = session.attempts;
    let accuracy = if attempts == 0 {
        None
    } else {
        // 分母是全部尝试（含已纠正的错误），纠正不会抬高准确率
        Some(f64::from(attempts - session.wrong_attempts.min(attempts)) / f64::from(attempts))
    };
    let (cpm, wpm) = if session.active_ms == 0 || correct == 0 {
        (None, None)
    } else {
        let minutes = session.active_ms as f64 / 60_000.0;
        let cpm = f64::from(correct) / minutes;
        (Some(cpm), Some(cpm / 5.0))
    };
    let progress = if total == 0 {
        0.0
    } else {
        f64::from(correct) / f64::from(total)
    };
    Score {
        correct,
        wrong_now,
        total,
        progress,
        accuracy,
        cpm,
        wpm,
        completed: total > 0 && correct == total && wrong_now == 0,
        pasted: session.pasted,
    }
}

#[cfg(test)]
#[path = "score_test.rs"]
mod tests;
