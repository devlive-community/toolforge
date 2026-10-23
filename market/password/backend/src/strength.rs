use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::generate::{DIGITS, LOWER, SYMBOLS, UPPER};

/// 离线快速哈希场景下的猜测速度（次/秒）
const GUESSES_PER_SECOND: f64 = 1e10;
const MAX_INPUT: usize = 1024;
const FOREVER_SECONDS: f64 = 3_155_760_000.0 * 1e6;
const KEYBOARD_ROWS: [&str; 4] = ["qwertyuiop", "asdfghjkl", "zxcvbnm", "1234567890"];

/// 最常见的弱密码（小写，每行一个）
const COMMON: &str = include_str!("common.txt");

fn is_common(password: &str) -> bool {
    let lower = password.to_lowercase();
    COMMON.lines().any(|line| line == lower)
}

#[derive(Deserialize)]
pub struct Args {
    password: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Classes {
    pub upper: bool,
    pub lower: bool,
    pub digits: bool,
    pub symbols: bool,
    pub other: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct CrackTime {
    /// instant / seconds / minutes / hours / days / years / centuries / forever
    pub unit: &'static str,
    pub value: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub length: usize,
    pub classes: Classes,
    pub pool_size: usize,
    /// 扣除重复、序列、常见密码后的有效熵（比特）
    pub entropy: f64,
    /// 0（极弱）到 4（很强）
    pub score: u8,
    pub crack_time: CrackTime,
    pub warnings: Vec<&'static str>,
}

pub fn entropy_bits(pool: usize, length: usize) -> f64 {
    if pool <= 1 || length == 0 {
        return 0.0;
    }
    ((pool as f64).log2() * length as f64 * 10.0).round() / 10.0
}

fn score(entropy: f64) -> u8 {
    match entropy {
        e if e < 28.0 => 0,
        e if e < 36.0 => 1,
        e if e < 60.0 => 2,
        e if e < 80.0 => 3,
        _ => 4,
    }
}

pub fn crack_time(entropy: f64) -> CrackTime {
    // 平均需要搜索一半的空间
    let seconds = 2f64.powf(entropy - 1.0) / GUESSES_PER_SECOND;
    let units: [(&str, f64); 6] = [
        ("seconds", 1.0),
        ("minutes", 60.0),
        ("hours", 3600.0),
        ("days", 86_400.0),
        ("years", 31_557_600.0),
        ("centuries", 3_155_760_000.0),
    ];
    if seconds < 1.0 {
        return CrackTime {
            unit: "instant",
            value: 0,
        };
    }
    // 超过一百万个世纪时数字已无意义
    if seconds >= FOREVER_SECONDS {
        return CrackTime {
            unit: "forever",
            value: 0,
        };
    }
    let (unit, size) = units
        .iter()
        .rev()
        .find(|(_, size)| seconds >= *size)
        .copied()
        .unwrap_or(units[0]);
    CrackTime {
        unit,
        value: (seconds / size).round() as u64,
    }
}

/// 统计属于重复（aaa）或连续序列（abc、321、qwe）的字符数
fn patterned_chars(chars: &[char]) -> (usize, bool, bool) {
    let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut marked = vec![false; chars.len()];
    let (mut repeated, mut sequence) = (false, false);
    for i in 2..lower.len() {
        let (a, b, c) = (lower[i - 2], lower[i - 1], lower[i]);
        let is_repeat = a == b && b == c;
        let step = |x: char, y: char| y as i32 - x as i32;
        let is_step = step(a, b) == step(b, c) && step(a, b).abs() == 1;
        let is_keyboard = KEYBOARD_ROWS.iter().any(|row| {
            let triple: String = [a, b, c].iter().collect();
            let reversed: String = [c, b, a].iter().collect();
            row.contains(&triple) || row.contains(&reversed)
        });
        if is_repeat || is_step || is_keyboard {
            repeated |= is_repeat;
            sequence |= is_step || is_keyboard;
            marked[i - 2..=i].iter_mut().for_each(|m| *m = true);
        }
    }
    (marked.iter().filter(|m| **m).count(), repeated, sequence)
}

pub fn analyze(args: Args) -> PluginResult<Report> {
    let password = args.password;
    if password.is_empty() {
        return Err(PluginError::new("password.empty"));
    }
    if password.len() > MAX_INPUT {
        return Err(PluginError::new("password.too_long").with("limit", MAX_INPUT));
    }
    let chars: Vec<char> = password.chars().collect();
    let classes = Classes {
        upper: chars.iter().any(|c| UPPER.contains(*c)),
        lower: chars.iter().any(|c| LOWER.contains(*c)),
        digits: chars.iter().any(|c| DIGITS.contains(*c)),
        symbols: chars
            .iter()
            .any(|c| SYMBOLS.contains(*c) || c.is_ascii_punctuation() || *c == ' '),
        other: chars.iter().any(|c| !c.is_ascii()),
    };
    let pool = [
        (classes.upper, 26),
        (classes.lower, 26),
        (classes.digits, 10),
        (classes.symbols, 33),
        (classes.other, 100),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, size)| size)
    .sum::<usize>();

    let mut warnings = Vec::new();
    let common = is_common(&password);
    let (patterned, repeated, sequence) = patterned_chars(&chars);
    if common {
        warnings.push("common");
    }
    if chars.len() < 12 {
        warnings.push("short");
    }
    if repeated {
        warnings.push("repeated");
    }
    if sequence {
        warnings.push("sequence");
    }
    let class_count = [
        classes.upper,
        classes.lower,
        classes.digits,
        classes.symbols,
        classes.other,
    ]
    .iter()
    .filter(|c| **c)
    .count();
    if class_count == 1 {
        warnings.push("single_class");
    }

    // 规律字符按 1/4 计入长度
    let effective = (chars.len() - patterned) as f64 + patterned as f64 / 4.0;
    let entropy = if common {
        (COMMON.lines().count() as f64).log2().round()
    } else {
        ((pool.max(2) as f64).log2() * effective * 10.0).round() / 10.0
    };

    Ok(Report {
        length: chars.len(),
        classes,
        pool_size: pool,
        entropy,
        score: score(entropy),
        crack_time: crack_time(entropy),
        warnings,
    })
}

#[cfg(test)]
#[path = "strength_test.rs"]
mod tests;
