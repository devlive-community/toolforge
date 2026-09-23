use std::time::Instant;

use fancy_regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::offsets::Utf16Offsets;

/// 返回给界面的最大匹配数
pub const MAX_MATCHES: usize = 1000;
/// 允许处理的最大文本（字节）
const MAX_INPUT: usize = 5 * 1024 * 1024;
/// 回溯上限，防止灾难性回溯卡死
const BACKTRACK_LIMIT: usize = 1_000_000;
/// 单个匹配 / 分组文本返回的最大字符数
const MAX_TEXT: usize = 500;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flags {
    #[serde(default)]
    pub case_insensitive: bool,
    #[serde(default)]
    pub multi_line: bool,
    #[serde(default)]
    pub dot_all: bool,
    #[serde(default)]
    pub extended: bool,
}

#[derive(Deserialize)]
pub struct TestArgs {
    pub pattern: String,
    #[serde(default)]
    pub flags: Flags,
    pub input: String,
}

#[derive(Deserialize)]
pub struct ReplaceArgs {
    pub pattern: String,
    #[serde(default)]
    pub flags: Flags,
    pub input: String,
    pub replacement: String,
}

#[derive(Debug, Serialize)]
pub struct Group {
    pub index: usize,
    pub name: Option<String>,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Match {
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub text: String,
    /// 与分组序号对应；未参与匹配的分组为 None
    pub groups: Vec<Option<Group>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestOutput {
    pub matches: Vec<Match>,
    pub count: usize,
    pub truncated: bool,
    pub group_names: Vec<Option<String>>,
    pub elapsed_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceOutput {
    pub output: String,
    pub count: usize,
    pub elapsed_ms: f64,
}

fn elapsed(start: Instant) -> f64 {
    (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_TEXT {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(MAX_TEXT).collect();
    out.push('…');
    out
}

fn runtime_error(err: fancy_regex::Error) -> PluginError {
    match err {
        fancy_regex::Error::RuntimeError(fancy_regex::RuntimeError::BacktrackLimitExceeded) => {
            PluginError::new("regex.backtrack_limit")
        }
        other => PluginError::new("regex.runtime").with("detail", other.to_string()),
    }
}

pub fn build(pattern: &str, flags: Flags) -> PluginResult<Regex> {
    if pattern.is_empty() {
        return Err(PluginError::new("regex.empty"));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(flags.case_insensitive)
        .multi_line(flags.multi_line)
        .dot_matches_new_line(flags.dot_all)
        .ignore_whitespace(flags.extended)
        .backtrack_limit(BACKTRACK_LIMIT)
        .build()
        .map_err(|e| PluginError::new("regex.invalid").with("detail", e.to_string()))
}

fn check_input(input: &str) -> PluginResult<()> {
    if input.len() > MAX_INPUT {
        return Err(PluginError::new("regex.input_too_large").with("limit", "5 MB"));
    }
    Ok(())
}

pub fn test(args: TestArgs) -> PluginResult<TestOutput> {
    check_input(&args.input)?;
    let regex = build(&args.pattern, args.flags)?;
    let start = Instant::now();
    let offsets = Utf16Offsets::new(&args.input);
    let group_names: Vec<Option<String>> = regex
        .capture_names()
        .map(|n| n.map(str::to_owned))
        .collect();

    let mut matches = Vec::new();
    let mut count = 0;
    for captures in regex.captures_iter(&args.input) {
        let captures = captures.map_err(runtime_error)?;
        count += 1;
        if matches.len() >= MAX_MATCHES {
            continue;
        }
        let whole = captures.get(0).expect("group 0 always participates");
        let groups = (1..captures.len())
            .map(|index| {
                captures.get(index).map(|m| Group {
                    index,
                    name: group_names.get(index).cloned().flatten(),
                    start: offsets.get(m.start()),
                    end: offsets.get(m.end()),
                    text: clip(m.as_str()),
                })
            })
            .collect();
        matches.push(Match {
            index: count,
            start: offsets.get(whole.start()),
            end: offsets.get(whole.end()),
            text: clip(whole.as_str()),
            groups,
        });
    }

    Ok(TestOutput {
        truncated: count > matches.len(),
        matches,
        count,
        group_names,
        elapsed_ms: elapsed(start),
    })
}

pub fn replace(args: ReplaceArgs) -> PluginResult<ReplaceOutput> {
    check_input(&args.input)?;
    let regex = build(&args.pattern, args.flags)?;
    let start = Instant::now();
    let mut count = 0;
    for m in regex.find_iter(&args.input) {
        m.map_err(runtime_error)?;
        count += 1;
    }
    let output = regex
        .try_replacen(&args.input, 0, args.replacement.as_str())
        .map_err(runtime_error)?
        .into_owned();
    Ok(ReplaceOutput {
        output,
        count,
        elapsed_ms: elapsed(start),
    })
}

#[cfg(test)]
#[path = "engine_test.rs"]
mod tests;
