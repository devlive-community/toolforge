use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

/// 阅读速度：英文 220 词 / 分钟，中日韩 400 字 / 分钟
const LATIN_WPM: f64 = 220.0;
const CJK_CPM: f64 = 400.0;
const TOP_WORDS: usize = 10;

#[derive(Deserialize)]
pub struct Args {
    input: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// 用户可见字符（字素簇），emoji 计为 1
    pub characters: usize,
    pub characters_no_spaces: usize,
    /// 所有词：中日韩按字计
    pub words: usize,
    pub cjk_characters: usize,
    pub latin_words: usize,
    pub lines: usize,
    pub non_empty_lines: usize,
    pub paragraphs: usize,
    pub sentences: usize,
    pub bytes: usize,
    pub reading_seconds: u64,
    pub top_words: Vec<(String, usize)>,
}

pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0xF900..=0xFAFF // 汉字
        | 0x3040..=0x309F | 0x30A0..=0x30FF // 假名
        | 0xAC00..=0xD7AF) // 谚文
}

pub fn collect(args: Args) -> Stats {
    let text = args.input.as_str();
    let graphemes = text.graphemes(true);
    let characters = graphemes.clone().count();
    let characters_no_spaces = graphemes
        .filter(|g| !g.chars().all(char::is_whitespace))
        .count();

    let words: Vec<&str> = text.unicode_words().collect();
    let cjk_characters = text.chars().filter(|c| is_cjk(*c)).count();
    let latin: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| !w.chars().any(is_cjk))
        .collect();

    let mut frequency: HashMap<String, usize> = HashMap::new();
    for word in &latin {
        if word.chars().count() > 1 && !word.chars().all(|c| c.is_ascii_digit()) {
            *frequency.entry(word.to_lowercase()).or_default() += 1;
        }
    }
    let mut top_words: Vec<(String, usize)> = frequency.into_iter().collect();
    top_words.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_words.truncate(TOP_WORDS);

    let lines: Vec<&str> = if text.is_empty() {
        Vec::new()
    } else {
        text.lines().collect()
    };
    let paragraphs = text.split("\n\n").filter(|p| !p.trim().is_empty()).count();
    let minutes = latin.len() as f64 / LATIN_WPM + cjk_characters as f64 / CJK_CPM;

    Stats {
        characters,
        characters_no_spaces,
        words: words.len(),
        cjk_characters,
        latin_words: latin.len(),
        non_empty_lines: lines.iter().filter(|l| !l.trim().is_empty()).count(),
        lines: lines.len(),
        paragraphs,
        // unicode-segmentation 对空字符串分句会 panic，需先判断
        sentences: if text.trim().is_empty() {
            0
        } else {
            text.unicode_sentences()
                .filter(|s| !s.trim().is_empty())
                .count()
        },
        bytes: text.len(),
        reading_seconds: (minutes * 60.0).ceil() as u64,
        top_words,
    }
}

#[cfg(test)]
#[path = "stats_test.rs"]
mod tests;
