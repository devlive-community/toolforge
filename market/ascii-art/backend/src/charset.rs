//! 符号梯度：内置预设与自定义符号串的校验。
//!
//! 梯度约定「由浅到深」：按墨量（覆盖密度）递增排列，首字符应最浅（通常是空格）。
//! 映射时亮像素取深符号（黑底白字终端里深符号显示为亮块），反相则反过来。

use tf_plugin_api::{PluginError, PluginResult};

/// 内置符号组：(id, 由浅到深的符号串)
pub const PRESETS: &[(&str, &str)] = &[
    ("classic", " .:-=+*#%@"),
    ("blocks", " ░▒▓█"),
    ("minimal", " .oO@"),
    ("dots", " ·:∴"),
    ("binary", " 01"),
];

pub fn preset_chars(id: &str) -> Option<&'static str> {
    PRESETS
        .iter()
        .find(|(key, _)| *key == id)
        .map(|(_, chars)| *chars)
}

/// 校验结果统计：重复符号数、被忽略的不可用字符数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomStats {
    pub duplicates: usize,
    pub skipped: usize,
}

/// 规范化自定义符号串：只保留可打印 ASCII（0x20-0x7E），按输入顺序去重。
/// 换行、制表符、控制字符与非 ASCII 字符都会被跳过并计数；
/// 有效字符不足两个时报 `ascii.bad_custom`。
pub fn normalize_custom(input: &str) -> PluginResult<(String, CustomStats)> {
    let mut out = String::with_capacity(input.len());
    let mut seen = [false; 95];
    let mut duplicates = 0usize;
    let mut skipped = 0usize;
    for ch in input.chars() {
        // 可打印 ASCII 之外的一律跳过：控制字符（含换行、制表）与非 ASCII
        if !('\u{20}'..='\u{7e}').contains(&ch) {
            skipped += 1;
            continue;
        }
        let slot = ch as usize - 0x20;
        if seen[slot] {
            duplicates += 1;
            continue;
        }
        seen[slot] = true;
        out.push(ch);
    }
    if out.chars().count() < 2 {
        return Err(PluginError::new("ascii.bad_custom").with(
            "reason",
            "需要至少两个不同的可打印 ASCII 字符 / need at least 2 distinct printable ASCII characters",
        ));
    }
    Ok((
        out,
        CustomStats {
            duplicates,
            skipped,
        },
    ))
}

#[cfg(test)]
#[path = "charset_test.rs"]
mod tests;
