//! 一次分析：解析版本列表、排序，并按范围逐个判断。

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::range::{self, Outcome};
use crate::version::{self, Identifier};

pub const MAX_VERSIONS: usize = 10_000;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    #[default]
    Asc,
    Desc,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 每行一个版本
    pub versions: String,
    #[serde(default)]
    pub range: String,
    #[serde(default)]
    pub include_prerelease: bool,
    #[serde(default)]
    pub order: Order,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// 输入中的行号（1 起始）
    pub line: usize,
    pub input: String,
    pub normalized: Option<String>,
    pub major: Option<u64>,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub prerelease: Vec<Identifier>,
    pub build: Vec<String>,
    pub error: Option<PluginError>,
    /// 未填写范围时为 None
    pub outcome: Option<Outcome>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    /// 合法版本按优先级排序（优先级相同的保持输入顺序），非法版本排在最后
    pub rows: Vec<Row>,
    pub valid: usize,
    pub invalid: usize,
    pub matched: Option<usize>,
    /// 标准化后的比较集合（|| 分隔的每一组）
    pub sets: Vec<String>,
    pub range_error: Option<PluginError>,
    /// 与相邻版本优先级相同（仅构建元数据不同）的行号
    pub equal_precedence: Vec<[usize; 2]>,
    pub csv: String,
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    let lines: Vec<(usize, &str)> = args
        .versions
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if lines.len() > MAX_VERSIONS {
        return Err(PluginError::new("semver.too_many").with("max", MAX_VERSIONS));
    }
    let range_text = args.range.trim();
    let (parsed_range, range_error) = if range_text.is_empty() {
        (None, None)
    } else {
        match range::parse(range_text) {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e)),
        }
    };

    let mut parsed: Vec<(usize, &str, PluginResult<version::Version>)> = lines
        .into_iter()
        .map(|(line, text)| (line, text, version::parse(text)))
        .collect();
    parsed.sort_by(|a, b| match (&a.2, &b.2) {
        (Ok(x), Ok(y)) => {
            let ord = x.precedence(y);
            if args.order == Order::Desc {
                ord.reverse()
            } else {
                ord
            }
        }
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.0.cmp(&b.0),
    });

    let mut equal_precedence = Vec::new();
    for pair in parsed.windows(2) {
        if let (Ok(x), Ok(y)) = (&pair[0].2, &pair[1].2)
            && x.precedence(y) == Ordering::Equal
        {
            equal_precedence.push([pair[0].0, pair[1].0]);
        }
    }

    let mut csv = String::from("line,version,normalized,result,detail\n");
    let mut rows = Vec::with_capacity(parsed.len());
    let (mut valid, mut invalid, mut matched) = (0, 0, 0);
    for (line, text, result) in parsed {
        let row = match result {
            Ok(v) => {
                valid += 1;
                let outcome = parsed_range
                    .as_ref()
                    .map(|r| r.test(&v, args.include_prerelease));
                if outcome == Some(Outcome::Match) {
                    matched += 1;
                }
                Row {
                    line,
                    input: text.to_owned(),
                    normalized: Some(v.to_string()),
                    major: Some(v.major),
                    minor: Some(v.minor),
                    patch: Some(v.patch),
                    prerelease: v.pre,
                    build: v.build,
                    error: None,
                    outcome,
                }
            }
            Err(error) => {
                invalid += 1;
                Row {
                    line,
                    input: text.to_owned(),
                    normalized: None,
                    major: None,
                    minor: None,
                    patch: None,
                    prerelease: Vec::new(),
                    build: Vec::new(),
                    error: Some(error),
                    outcome: None,
                }
            }
        };
        let (result, detail) = match (&row.error, &row.outcome) {
            (Some(e), _) => ("invalid", e.code.clone()),
            (None, Some(Outcome::Match)) => ("match", String::new()),
            (None, Some(Outcome::Failed { comparator })) => ("no-match", comparator.clone()),
            (None, Some(Outcome::Prerelease)) => ("no-match", "prerelease".to_owned()),
            (None, None) => ("", String::new()),
        };
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            line,
            csv_field(text),
            csv_field(row.normalized.as_deref().unwrap_or("")),
            result,
            csv_field(&detail)
        ));
        rows.push(row);
    }

    Ok(Output {
        rows,
        valid,
        invalid,
        matched: parsed_range.as_ref().map(|_| matched),
        sets: parsed_range.map(|r| r.normalized()).unwrap_or_default(),
        range_error,
        equal_precedence,
        csv,
    })
}

#[cfg(test)]
#[path = "analyze_test.rs"]
mod tests;
