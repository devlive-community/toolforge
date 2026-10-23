use rand::seq::SliceRandom;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::strength::entropy_bits;

pub const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
pub const DIGITS: &str = "0123456789";
pub const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.<>/?~";
/// 容易混淆的字符
pub const AMBIGUOUS: &str = "0O1lI|`'\"";
const MAX_LENGTH: usize = 256;
const MAX_COUNT: usize = 500;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub length: usize,
    #[serde(default = "default_count")]
    pub count: usize,
    #[serde(default = "default_true")]
    pub upper: bool,
    #[serde(default = "default_true")]
    pub lower: bool,
    #[serde(default = "default_true")]
    pub digits: bool,
    #[serde(default)]
    pub symbols: bool,
    #[serde(default)]
    pub exclude_ambiguous: bool,
    /// 额外排除的字符
    #[serde(default)]
    pub exclude: String,
    /// 每个选中的字符类别至少出现一次
    #[serde(default = "default_true")]
    pub require_each: bool,
}

fn default_count() -> usize {
    1
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub passwords: Vec<String>,
    pub pool_size: usize,
    pub entropy: f64,
}

/// 按选项得到各字符类别（已去除排除字符），空类别被丢弃
pub fn classes(args: &Args) -> Vec<Vec<char>> {
    [
        (args.upper, UPPER),
        (args.lower, LOWER),
        (args.digits, DIGITS),
        (args.symbols, SYMBOLS),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, set)| {
        set.chars()
            .filter(|c| !(args.exclude_ambiguous && AMBIGUOUS.contains(*c)))
            .filter(|c| !args.exclude.contains(*c))
            .collect::<Vec<_>>()
    })
    .filter(|set| !set.is_empty())
    .collect()
}

fn one(
    classes: &[Vec<char>],
    pool: &[char],
    length: usize,
    require_each: bool,
    rng: &mut impl Rng,
) -> String {
    let mut chars: Vec<char> = Vec::with_capacity(length);
    if require_each {
        for class in classes {
            chars.push(class[rng.random_range(0..class.len())]);
        }
    }
    while chars.len() < length {
        chars.push(pool[rng.random_range(0..pool.len())]);
    }
    // 打乱位置，避免必选字符总在开头
    chars.shuffle(rng);
    chars.into_iter().collect()
}

pub fn generate_with(args: Args, rng: &mut impl Rng) -> PluginResult<Output> {
    if !(1..=MAX_LENGTH).contains(&args.length) {
        return Err(PluginError::new("password.invalid_length").with("max", MAX_LENGTH));
    }
    if !(1..=MAX_COUNT).contains(&args.count) {
        return Err(PluginError::new("password.invalid_count").with("max", MAX_COUNT));
    }
    let classes = classes(&args);
    if classes.is_empty() {
        return Err(PluginError::new("password.no_charset"));
    }
    if args.require_each && classes.len() > args.length {
        return Err(PluginError::new("password.too_short_for_classes").with("min", classes.len()));
    }
    let pool: Vec<char> = classes.iter().flatten().copied().collect();
    let passwords = (0..args.count)
        .map(|_| one(&classes, &pool, args.length, args.require_each, rng))
        .collect();
    Ok(Output {
        passwords,
        pool_size: pool.len(),
        entropy: entropy_bits(pool.len(), args.length),
    })
}

/// 使用线程本地的密码学安全随机数生成器
pub fn generate(args: Args) -> PluginResult<Output> {
    generate_with(args, &mut rand::rng())
}

#[cfg(test)]
#[path = "generate_test.rs"]
mod tests;
