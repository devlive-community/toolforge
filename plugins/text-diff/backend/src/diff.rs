use std::time::Instant;

use serde::{Deserialize, Serialize};
use similar::{Algorithm, ChangeTag, DiffOp, TextDiff, capture_diff_slices};
use tf_plugin_api::{PluginError, PluginResult};

const MAX_INPUT: usize = 5 * 1024 * 1024;
/// 并排视图最多返回的行数
pub const MAX_ROWS: usize = 20_000;
const CONTEXT: usize = 3;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Lines,
    Words,
    Chars,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Algo {
    #[default]
    Myers,
    Patience,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub left: String,
    pub right: String,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub algorithm: Algo,
    #[serde(default)]
    pub ignore_case: bool,
    /// 比较时忽略行首尾空白并合并连续空白
    #[serde(default)]
    pub ignore_whitespace: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Tag {
    Equal,
    Delete,
    Insert,
    Change,
}

/// 行内片段：emphasized 表示该片段是行内具体改动的部分
#[derive(Debug, Serialize, PartialEq)]
pub struct Segment {
    pub text: String,
    pub emphasized: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Line {
    pub number: usize,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub tag: Tag,
    pub left: Option<Line>,
    pub right: Option<Line>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Token {
    pub tag: Tag,
    pub text: String,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub unchanged: usize,
    /// 0–1
    pub similarity: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub mode: Mode,
    pub identical: bool,
    pub rows: Vec<Row>,
    pub tokens: Vec<Token>,
    pub truncated: bool,
    pub stats: Stats,
    pub unified: String,
    pub elapsed_ms: f64,
}

fn algorithm(algo: Algo) -> Algorithm {
    match algo {
        Algo::Myers => Algorithm::Myers,
        Algo::Patience => Algorithm::Patience,
    }
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

fn normalize(line: &str, args: &Args) -> String {
    let mut key = if args.ignore_whitespace {
        line.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        line.to_owned()
    };
    if args.ignore_case {
        key = key.to_lowercase();
    }
    key
}

fn plain(number: usize, text: &str) -> Line {
    Line {
        number,
        segments: vec![Segment {
            text: text.to_owned(),
            emphasized: false,
        }],
    }
}

/// 一对被修改的行：按词对比并标出具体改动的片段
fn inline(left_no: usize, left: &str, right_no: usize, right: &str) -> (Line, Line) {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .diff_words(left, right);
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for change in diff.iter_all_changes() {
        let text = change.value().to_owned();
        match change.tag() {
            ChangeTag::Equal => {
                l.push(Segment {
                    text: text.clone(),
                    emphasized: false,
                });
                r.push(Segment {
                    text,
                    emphasized: false,
                });
            }
            ChangeTag::Delete => l.push(Segment {
                text,
                emphasized: true,
            }),
            ChangeTag::Insert => r.push(Segment {
                text,
                emphasized: true,
            }),
        }
    }
    (
        Line {
            number: left_no,
            segments: merge(l),
        },
        Line {
            number: right_no,
            segments: merge(r),
        },
    )
}

fn merge(segments: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    for segment in segments {
        match out.last_mut() {
            Some(last) if last.emphasized == segment.emphasized => {
                last.text.push_str(&segment.text)
            }
            _ => out.push(segment),
        }
    }
    out
}

fn line_diff(args: &Args) -> (Vec<Row>, Stats, bool) {
    let (left, right) = (split_lines(&args.left), split_lines(&args.right));
    let left_keys: Vec<String> = left.iter().map(|l| normalize(l, args)).collect();
    let right_keys: Vec<String> = right.iter().map(|l| normalize(l, args)).collect();
    let ops = capture_diff_slices(algorithm(args.algorithm), &left_keys, &right_keys);

    let mut rows = Vec::new();
    let mut stats = Stats::default();
    let push = |row: Row, rows: &mut Vec<Row>| {
        if rows.len() < MAX_ROWS {
            rows.push(row);
        }
    };

    for op in ops {
        match op {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                stats.unchanged += len;
                for i in 0..len {
                    push(
                        Row {
                            tag: Tag::Equal,
                            left: Some(plain(old_index + i + 1, left[old_index + i])),
                            right: Some(plain(new_index + i + 1, right[new_index + i])),
                        },
                        &mut rows,
                    );
                }
            }
            DiffOp::Delete {
                old_index, old_len, ..
            } => {
                stats.removed += old_len;
                for (i, line) in left.iter().enumerate().skip(old_index).take(old_len) {
                    push(
                        Row {
                            tag: Tag::Delete,
                            left: Some(plain(i + 1, line)),
                            right: None,
                        },
                        &mut rows,
                    );
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                stats.added += new_len;
                for (i, line) in right.iter().enumerate().skip(new_index).take(new_len) {
                    push(
                        Row {
                            tag: Tag::Insert,
                            left: None,
                            right: Some(plain(i + 1, line)),
                        },
                        &mut rows,
                    );
                }
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let paired = old_len.min(new_len);
                stats.changed += paired;
                stats.removed += old_len - paired;
                stats.added += new_len - paired;
                for i in 0..paired {
                    let (l, r) = inline(
                        old_index + i + 1,
                        left[old_index + i],
                        new_index + i + 1,
                        right[new_index + i],
                    );
                    push(
                        Row {
                            tag: Tag::Change,
                            left: Some(l),
                            right: Some(r),
                        },
                        &mut rows,
                    );
                }
                for (i, line) in left
                    .iter()
                    .enumerate()
                    .skip(old_index + paired)
                    .take(old_len - paired)
                {
                    push(
                        Row {
                            tag: Tag::Delete,
                            left: Some(plain(i + 1, line)),
                            right: None,
                        },
                        &mut rows,
                    );
                }
                for (i, line) in right
                    .iter()
                    .enumerate()
                    .skip(new_index + paired)
                    .take(new_len - paired)
                {
                    push(
                        Row {
                            tag: Tag::Insert,
                            left: None,
                            right: Some(plain(i + 1, line)),
                        },
                        &mut rows,
                    );
                }
            }
        }
    }
    let total = left.len() + right.len();
    stats.similarity = if total == 0 {
        1.0
    } else {
        (2 * stats.unchanged) as f32 / total as f32
    };
    let truncated = rows.len() >= MAX_ROWS;
    (rows, stats, truncated)
}

fn token_diff(args: &Args) -> (Vec<Token>, Stats) {
    let mut config = TextDiff::configure();
    config.algorithm(algorithm(args.algorithm));
    let diff = match args.mode {
        Mode::Chars => config.diff_chars(args.left.as_str(), args.right.as_str()),
        _ => config.diff_words(args.left.as_str(), args.right.as_str()),
    };
    let mut tokens: Vec<Token> = Vec::new();
    let mut stats = Stats {
        similarity: diff.ratio(),
        ..Stats::default()
    };
    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Equal => Tag::Equal,
            ChangeTag::Delete => Tag::Delete,
            ChangeTag::Insert => Tag::Insert,
        };
        let is_space = change.value().chars().all(char::is_whitespace);
        match tag {
            Tag::Delete if !is_space => stats.removed += 1,
            Tag::Insert if !is_space => stats.added += 1,
            Tag::Equal if !is_space => stats.unchanged += 1,
            _ => {}
        }
        match tokens.last_mut() {
            Some(last) if last.tag == tag => last.text.push_str(change.value()),
            _ => tokens.push(Token {
                tag,
                text: change.value().to_owned(),
            }),
        }
    }
    (tokens, stats)
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.left.len() > MAX_INPUT || args.right.len() > MAX_INPUT {
        return Err(PluginError::new("diff.too_large").with("limit", "5 MB"));
    }
    let start = Instant::now();
    let unified = TextDiff::from_lines(args.left.as_str(), args.right.as_str())
        .unified_diff()
        .context_radius(CONTEXT)
        .header("left", "right")
        .to_string();

    let (rows, tokens, stats, truncated) = match args.mode {
        Mode::Lines => {
            let (rows, stats, truncated) = line_diff(&args);
            (rows, Vec::new(), stats, truncated)
        }
        Mode::Words | Mode::Chars => {
            let (tokens, stats) = token_diff(&args);
            (Vec::new(), tokens, stats, false)
        }
    };
    let identical = stats.added == 0 && stats.removed == 0 && stats.changed == 0;

    Ok(Output {
        mode: args.mode,
        identical,
        rows,
        tokens,
        truncated,
        stats,
        unified,
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

#[cfg(test)]
#[path = "diff_test.rs"]
mod tests;
