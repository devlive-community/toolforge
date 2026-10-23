use std::cmp::Ordering;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    None,
    Asc,
    Desc,
    Natural,
    Length,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub input: String,
    #[serde(default)]
    pub trim: bool,
    #[serde(default)]
    pub remove_empty: bool,
    #[serde(default)]
    pub dedupe: bool,
    /// 去重与排序时忽略大小写
    #[serde(default)]
    pub ignore_case: bool,
    #[serde(default)]
    pub sort: Sort,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub number: bool,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    pub before: usize,
    pub after: usize,
    pub duplicates: usize,
    pub empty: usize,
}

/// 自然排序：数字段按数值比较（file2 < file10）
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (da, db) = (take(&mut a), take(&mut b));
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                let ordering = ta
                    .len()
                    .cmp(&tb.len())
                    .then_with(|| ta.cmp(tb))
                    .then_with(|| da.len().cmp(&db.len()));
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(x), Some(y)) => {
                let ordering = x.cmp(&y);
                if ordering != Ordering::Equal {
                    return ordering;
                }
                a.next();
                b.next();
            }
        }
    }
}

pub fn process(args: Args) -> Output {
    let mut lines: Vec<String> = args
        .input
        .split('\n')
        .map(|l| l.trim_end_matches('\r').to_owned())
        .collect();
    let before = lines.len();
    let key = |line: &str| {
        if args.ignore_case {
            line.to_lowercase()
        } else {
            line.to_owned()
        }
    };

    if args.trim {
        lines
            .iter_mut()
            .for_each(|line| *line = line.trim().to_owned());
    }
    let mut empty = 0;
    if args.remove_empty {
        let count = lines.len();
        lines.retain(|line| !line.trim().is_empty());
        empty = count - lines.len();
    }
    let mut duplicates = 0;
    if args.dedupe {
        let mut seen = HashSet::new();
        let count = lines.len();
        lines.retain(|line| seen.insert(key(line)));
        duplicates = count - lines.len();
    }
    match args.sort {
        Sort::None => {}
        Sort::Asc => lines.sort_by_key(|line| key(line)),
        Sort::Desc => lines.sort_by_key(|line| std::cmp::Reverse(key(line))),
        Sort::Natural => lines.sort_by(|a, b| natural_cmp(&key(a), &key(b))),
        Sort::Length => lines.sort_by_key(|line| line.chars().count()),
    }
    if args.reverse {
        lines.reverse();
    }
    if args.shuffle {
        fastrand::shuffle(&mut lines);
    }
    let width = lines.len().to_string().len();
    let output = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let number = if args.number {
                format!("{:>width$}. ", index + 1)
            } else {
                String::new()
            };
            format!("{number}{}{line}{}", args.prefix, args.suffix)
        })
        .collect::<Vec<_>>()
        .join("\n");

    Output {
        output,
        before,
        after: lines.len(),
        duplicates,
        empty,
    }
}

#[cfg(test)]
#[path = "lines_test.rs"]
mod tests;
