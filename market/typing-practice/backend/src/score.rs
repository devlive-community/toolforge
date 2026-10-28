//! 打字成绩计算：逐字对照、错误尝试计数、准确率与速度。
//!
//! 口径（写在这里，界面与记忆文档引用同一份说明）：
//! - 「字符」指可见字符，即 Unicode 扩展字形簇：一个 emoji（含肤色、ZWJ 组合）或
//!   带组合符号的字母都只算一个字符，不会拆成多次错误。
//! - 换行：目标与输入中的 `\r\n` 统一视为 `\n`；除此之外不做任何替换，
//!   全角/半角标点、同形字、空格都按原样逐字比较。
//! - 增量：本次输入与上次输入的公共前缀之后的字符都算新提交的「尝试」；
//!   多字提交（如中文输入法一次上屏多个字）按真实字数计，选中替换也计入新尝试。
//!   退格只清除画面上的字符，不删除已经发生的错误尝试。
//! - 超出目标末尾的字符算错误尝试，也算「当前未改错误」，需删掉才能完成。
//! - 准确率 = (全部尝试 − 错误尝试) / 全部尝试；全部尝试为 0 时为 None（不显示）。
//! - CPM（每分钟正确字符数）= 当前正确字符数 / 有效分钟；WPM = CPM / 5，仅英文展示。
//! - 用时为 0 或没有正确字符时，速度为 None，避免产生无效或无穷大的速度。
//! - 过关 = 完成全文（无未改错误）且准确率 ≥ `PASS_ACCURACY`。

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

/// 关卡过关的准确率门槛
pub const PASS_ACCURACY: f64 = 0.9;
/// 练习文本的可见字符上限（内置与自定义共用），限制计算量与存档大小
pub const MAX_TEXT: usize = 5000;

/// 按可见字符（扩展字形簇）切分，并把 `\r\n` 统一为 `\n`。
pub fn graphemes(text: &str) -> Vec<&str> {
    text.graphemes(true)
        .map(|g| if g == "\r\n" { "\n" } else { g })
        .collect()
}

/// 一次提交的输入：练习区上一次与当前的完整文本。
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// 上一次已提交给后端的文本（新局为空）
    pub prev: String,
    /// 练习区当前全部文本
    pub typed: String,
    /// 是否由粘贴或拖放导入产生（前端检测到 paste / drop 事件时置 true）
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
    /// 画面上与目标一致的字符数
    pub correct: u32,
    /// 画面上仍未改正的错误字符数（含超出末尾的字符）
    pub wrong_now: u32,
    /// 目标文本的可见字符数
    pub total: u32,
    pub progress: f64,
    pub accuracy: Option<f64>,
    pub cpm: Option<f64>,
    pub wpm: Option<f64>,
    pub completed: bool,
    /// 完成且准确率达到门槛
    pub passed: bool,
    pub pasted: bool,
    /// 每个已输入位置是否正确（按可见字符），供界面逐字着色
    pub marks: Vec<bool>,
    /// 本次新提交中打错的目标字符（易错字符统计用，不含超出末尾的位置）
    pub missed: Vec<String>,
}

/// 应用一次提交，返回更新后的 Session 与当前成绩。`elapsed_ms` 是本局有效输入时长。
pub fn apply(
    target: &str,
    session: &Session,
    snapshot: &Snapshot,
    elapsed_ms: u64,
) -> (Session, Score) {
    let target = graphemes(target);
    let prev = graphemes(&snapshot.prev);
    let typed = graphemes(&snapshot.typed);

    // 公共前缀之后的都是新提交：末尾追加、多字提交、选中替换都按真实字数计
    let start = prev.iter().zip(&typed).take_while(|(a, b)| a == b).count();
    let mut missed = Vec::new();
    let mut wrong_added = 0u32;
    for (i, g) in typed.iter().enumerate().skip(start) {
        match target.get(i) {
            Some(expected) if expected == g => {}
            Some(expected) => {
                wrong_added += 1;
                missed.push((*expected).to_string());
            }
            None => wrong_added += 1,
        }
    }
    let added = count(typed.len() - start);

    let mut next = session.clone();
    next.attempts = next.attempts.saturating_add(added);
    // 错误尝试只增不减：之后退格纠正也不减少
    next.wrong_attempts = next.wrong_attempts.saturating_add(wrong_added);
    next.pasted = next.pasted || snapshot.pasted;
    next.active_ms = elapsed_ms;

    let marks: Vec<bool> = typed
        .iter()
        .enumerate()
        .map(|(i, g)| target.get(i) == Some(g))
        .collect();
    let correct = count(marks.iter().filter(|m| **m).count());
    let wrong_now = count(marks.len()) - correct;
    let score = build_score(
        count(target.len()),
        correct,
        wrong_now,
        &next,
        marks,
        missed,
    );
    (next, score)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn build_score(
    total: u32,
    correct: u32,
    wrong_now: u32,
    session: &Session,
    marks: Vec<bool>,
    missed: Vec<String>,
) -> Score {
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
    let completed = total > 0 && correct == total && wrong_now == 0;
    Score {
        correct,
        wrong_now,
        total,
        progress,
        accuracy,
        cpm,
        wpm,
        completed,
        passed: completed && accuracy.is_some_and(|a| a >= PASS_ACCURACY),
        pasted: session.pasted,
        marks,
        missed,
    }
}

#[cfg(test)]
#[path = "score_test.rs"]
mod tests;
