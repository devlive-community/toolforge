//! 密码短语：从内置离线词表中用密码学安全随机数抽词组合。

use std::sync::OnceLock;

use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// EFF 大词表（CC BY 3.0），已去掉含连字符的词
const EFF_LARGE: &str = include_str!("eff_large.txt");
/// 常用中文词与无声调拼音（拼音唯一），每行「词 拼音」
const ZH_WORDS: &str = include_str!("zh_words.txt");
const MIN_WORDS: usize = 3;
const MAX_WORDS: usize = 20;
const MAX_COUNT: usize = 500;
const MAX_SEPARATOR: usize = 8;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Wordlist {
    #[default]
    En,
    /// 输出拼音，并附带对应中文帮助记忆
    Zh,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Case {
    #[default]
    Lower,
    Upper,
    /// 首字母大写
    Title,
    /// 每个词随机全大写或全小写
    Random,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub words: usize,
    #[serde(default = "one")]
    pub count: usize,
    #[serde(default)]
    pub wordlist: Wordlist,
    #[serde(default = "dash")]
    pub separator: String,
    #[serde(default)]
    pub case: Case,
    /// 在随机一个词后追加一位数字
    #[serde(default)]
    pub append_digit: bool,
}

fn one() -> usize {
    1
}

fn dash() -> String {
    "-".into()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub passwords: Vec<String>,
    /// 中文词表时每条密码对应的中文词（空格分隔）
    pub hints: Option<Vec<String>>,
    pub wordlist_size: usize,
    pub entropy: f64,
}

/// (拼写, 中文) 列表
fn words(list: Wordlist) -> &'static [(&'static str, Option<&'static str>)] {
    static EN: OnceLock<Vec<(&str, Option<&str>)>> = OnceLock::new();
    static ZH: OnceLock<Vec<(&str, Option<&str>)>> = OnceLock::new();
    match list {
        Wordlist::En => EN.get_or_init(|| EFF_LARGE.lines().map(|w| (w, None)).collect()),
        Wordlist::Zh => ZH.get_or_init(|| {
            ZH_WORDS
                .lines()
                .filter_map(|line| line.split_once(' '))
                .map(|(word, pinyin)| (pinyin, Some(word)))
                .collect()
        }),
    }
}

pub fn entropy(list_size: usize, words: usize, append_digit: bool, case: Case) -> f64 {
    let mut bits = (list_size as f64).log2() * words as f64;
    if append_digit {
        bits += (10.0 * words as f64).log2();
    }
    if case == Case::Random {
        bits += words as f64;
    }
    (bits * 10.0).round() / 10.0
}

fn apply_case(word: &str, case: Case, rng: &mut impl Rng) -> String {
    match case {
        Case::Lower => word.to_string(),
        Case::Upper => word.to_ascii_uppercase(),
        Case::Title => {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        }
        Case::Random => {
            if rng.random_bool(0.5) {
                word.to_ascii_uppercase()
            } else {
                word.to_string()
            }
        }
    }
}

pub fn generate_with(args: Args, rng: &mut impl Rng) -> PluginResult<Output> {
    if !(MIN_WORDS..=MAX_WORDS).contains(&args.words) {
        return Err(PluginError::new("password.invalid_words")
            .with("min", MIN_WORDS)
            .with("max", MAX_WORDS));
    }
    if !(1..=MAX_COUNT).contains(&args.count) {
        return Err(PluginError::new("password.invalid_count").with("max", MAX_COUNT));
    }
    if args.separator.chars().count() > MAX_SEPARATOR {
        return Err(PluginError::new("password.invalid_separator").with("max", MAX_SEPARATOR));
    }
    let list = words(args.wordlist);
    let mut passwords = Vec::with_capacity(args.count);
    let mut hints = Vec::with_capacity(args.count);
    for _ in 0..args.count {
        let picked: Vec<_> = (0..args.words)
            .map(|_| list[rng.random_range(0..list.len())])
            .collect();
        let digit_at = args
            .append_digit
            .then(|| (rng.random_range(0..args.words), rng.random_range(0..10u8)));
        let parts: Vec<String> = picked
            .iter()
            .enumerate()
            .map(|(index, (spelling, _))| {
                let mut word = apply_case(spelling, args.case, rng);
                if let Some((at, digit)) = digit_at
                    && at == index
                {
                    word.push(char::from(b'0' + digit));
                }
                word
            })
            .collect();
        passwords.push(parts.join(&args.separator));
        hints.push(
            picked
                .iter()
                .filter_map(|(_, hint)| *hint)
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    Ok(Output {
        passwords,
        hints: (args.wordlist == Wordlist::Zh).then_some(hints),
        wordlist_size: list.len(),
        entropy: entropy(list.len(), args.words, args.append_digit, args.case),
    })
}

/// 使用线程本地的密码学安全随机数生成器
pub fn generate(args: Args) -> PluginResult<Output> {
    generate_with(args, &mut rand::rng())
}

#[cfg(test)]
#[path = "passphrase_test.rs"]
mod tests;
