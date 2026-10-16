//! 报告概览（文件与目录汇总）、单文件详情（按行命中、分支、未覆盖区段与源码诊断）与导出。

use std::collections::BTreeMap;

use data_encoding::{BASE64, BASE64_NOPAD};
use md5::{Digest, Md5};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::lcov::{Diagnostic, FileCov, Metrics, Report};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    #[serde(flatten)]
    pub metrics: Metrics,
    /// 目录下的文件数（文件为 1）
    pub files: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub token: u64,
    pub version: Option<String>,
    pub tests: Vec<String>,
    pub records: usize,
    pub totals: Metrics,
    pub files: Vec<Entry>,
    pub dirs: Vec<Entry>,
    /// 所有文件路径的公共目录前缀，界面显示时可省略
    pub common_prefix: String,
    pub diagnostics: Vec<Diagnostic>,
    pub diagnostics_dropped: usize,
    pub ignored: BTreeMap<String, usize>,
}

/// 把 \ 统一为 /，便于按目录汇总
pub fn normalize_separators(path: &str) -> String {
    path.replace('\\', "/")
}

fn parent_dirs(path: &str) -> Vec<String> {
    let normalized = normalize_separators(path);
    let mut out = Vec::new();
    let mut end = normalized.len();
    while let Some(slash) = normalized[..end].rfind('/') {
        if slash == 0 {
            out.push("/".to_owned());
            break;
        }
        out.push(normalized[..slash].to_owned());
        end = slash;
    }
    out
}

fn common_prefix(paths: &[String]) -> String {
    let mut iter = paths.iter().map(|p| normalize_separators(p));
    let Some(first) = iter.next() else {
        return String::new();
    };
    let mut prefix = first
        .rfind('/')
        .map_or(String::new(), |i| first[..=i].to_owned());
    for path in iter {
        while !prefix.is_empty() && !path.starts_with(&prefix) {
            prefix.pop();
            prefix = prefix
                .rfind('/')
                .map_or(String::new(), |i| prefix[..=i].to_owned());
        }
    }
    prefix
}

pub fn overview(report: &Report, token: u64) -> Overview {
    let mut totals = Metrics::default();
    let mut dirs: BTreeMap<String, (Metrics, usize)> = BTreeMap::new();
    let files: Vec<Entry> = report
        .files
        .iter()
        .map(|(path, cov)| {
            let metrics = cov.metrics();
            totals.add(&metrics);
            for dir in parent_dirs(path) {
                let entry = dirs.entry(dir).or_default();
                entry.0.add(&metrics);
                entry.1 += 1;
            }
            Entry {
                path: path.clone(),
                metrics,
                files: 1,
            }
        })
        .collect();
    let paths: Vec<String> = report.files.keys().cloned().collect();
    Overview {
        token,
        version: report.version.clone(),
        tests: report.tests.clone(),
        records: report.records,
        totals,
        common_prefix: common_prefix(&paths),
        files,
        dirs: dirs
            .into_iter()
            .map(|(path, (metrics, files))| Entry {
                path,
                metrics,
                files,
            })
            .collect(),
        diagnostics: report.diagnostics.clone(),
        diagnostics_dropped: report.diagnostics_dropped,
        ignored: report.ignored.clone(),
    }
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineView {
    pub line: u32,
    /// 源码文本（提供源码时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 命中次数；不是可执行行时为 None
    pub hits: Option<u64>,
    /// 这一行的分支：已执行 / 总数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branches: Option<(u64, u64)>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionView {
    pub name: String,
    pub line: Option<u32>,
    pub end_line: Option<u32>,
    pub hits: u64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchView {
    pub line: u32,
    pub block: String,
    pub branch: String,
    /// None 表示所在代码块未执行（`-`）
    pub taken: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceIssue {
    /// beyondSource / checksumMismatch
    pub kind: &'static str,
    pub line: u32,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub path: String,
    /// 不展开到顶层：避免与逐行数据 lines 重名
    pub metrics: Metrics,
    pub tests: Vec<String>,
    pub lines: Vec<LineView>,
    pub functions: Vec<FunctionView>,
    pub branches: Vec<BranchView>,
    /// 连续未命中的可执行行区段
    pub uncovered: Vec<Range>,
    pub source_issues: Vec<SourceIssue>,
    /// 源码诊断的总数（列表最多返回 MAX_SOURCE_ISSUES 条）
    pub source_issue_count: usize,
}

const MAX_SOURCE_ISSUES: usize = 200;

/// geninfo 的校验和：源码行（不含换行）的 MD5，base64 编码，可能省略填充
fn checksum_matches(text: &str, expected: &str) -> bool {
    let digest = Md5::digest(text.as_bytes());
    expected == BASE64.encode(&digest) || expected == BASE64_NOPAD.encode(&digest)
}

pub fn detail(path: &str, cov: &FileCov, source: Option<&str>) -> Detail {
    let source_lines: Option<Vec<&str>> = source.map(|s| s.lines().collect());
    let mut branch_counts: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    for ((line, _, _), taken) in &cov.branches {
        let entry = branch_counts.entry(*line).or_default();
        entry.1 += 1;
        if taken.is_some_and(|n| n > 0) {
            entry.0 += 1;
        }
    }

    let mut issues = Vec::new();
    let mut issue_count = 0;
    let mut issue = |kind, line| {
        issue_count += 1;
        if issues.len() < MAX_SOURCE_ISSUES {
            issues.push(SourceIssue { kind, line });
        }
    };
    let lines = match &source_lines {
        Some(text) => {
            for (line, data) in &cov.lines {
                match text.get(*line as usize - 1).filter(|_| *line > 0) {
                    None => issue("beyondSource", *line),
                    Some(content) => {
                        if let Some(sum) = &data.checksum
                            && !checksum_matches(content, sum)
                        {
                            issue("checksumMismatch", *line);
                        }
                    }
                }
            }
            text.iter()
                .enumerate()
                .map(|(i, content)| {
                    let n = i as u32 + 1;
                    LineView {
                        line: n,
                        text: Some((*content).to_owned()),
                        hits: cov.lines.get(&n).map(|l| l.hits),
                        branches: branch_counts.get(&n).copied(),
                    }
                })
                .collect()
        }
        None => cov
            .lines
            .iter()
            .map(|(line, data)| LineView {
                line: *line,
                text: None,
                hits: Some(data.hits),
                branches: branch_counts.get(line).copied(),
            })
            .collect(),
    };

    let mut uncovered: Vec<Range> = Vec::new();
    for (line, data) in &cov.lines {
        if data.hits > 0 {
            continue;
        }
        match uncovered.last_mut() {
            // 中间只隔着非可执行行时也视为同一段
            Some(range)
                if cov
                    .lines
                    .range(range.end + 1..*line)
                    .all(|(_, l)| l.hits == 0) =>
            {
                range.end = *line;
            }
            _ => uncovered.push(Range {
                start: *line,
                end: *line,
            }),
        }
    }

    let mut functions: Vec<FunctionView> = cov
        .functions
        .iter()
        .map(|(name, f)| FunctionView {
            name: name.clone(),
            line: f.line,
            end_line: f.end_line,
            hits: f.hits,
        })
        .collect();
    functions.sort_by_key(|f| (f.line.unwrap_or(u32::MAX), f.name.clone()));

    Detail {
        path: path.to_owned(),
        metrics: cov.metrics(),
        tests: cov.tests.iter().cloned().collect(),
        lines,
        functions,
        branches: cov
            .branches
            .iter()
            .map(|((line, block, branch), taken)| BranchView {
                line: *line,
                block: block.clone(),
                branch: branch.clone(),
                taken: *taken,
            })
            .collect(),
        uncovered,
        source_issues: issues,
        source_issue_count: issue_count,
    }
}

/// 百分比文本；分母为 0 时为 N/A
pub fn percent(counts: crate::lcov::Counts) -> String {
    if counts.found == 0 {
        "N/A".to_owned()
    } else {
        format!("{:.2}%", counts.hit as f64 * 100.0 / counts.found as f64)
    }
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Markdown,
}

pub fn export(report: &Report, format: ExportFormat) -> PluginResult<String> {
    let overview = overview(report, 0);
    let mut out = String::new();
    match format {
        ExportFormat::Csv => {
            out.push_str("file,lines_hit,lines_found,lines_pct,functions_hit,functions_found,functions_pct,branches_hit,branches_found,branches_pct,branches_not_executed\n");
            for file in overview.files.iter() {
                let m = &file.metrics;
                let fields = [
                    file.path.clone(),
                    m.lines.hit.to_string(),
                    m.lines.found.to_string(),
                    percent(m.lines),
                    m.functions.hit.to_string(),
                    m.functions.found.to_string(),
                    percent(m.functions),
                    m.branches.hit.to_string(),
                    m.branches.found.to_string(),
                    percent(m.branches),
                    m.branches_not_executed.to_string(),
                ];
                let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
                out.push_str(&line.join(","));
                out.push('\n');
            }
        }
        ExportFormat::Markdown => {
            let t = &overview.totals;
            out.push_str("| | Lines | Functions | Branches |\n|---|---:|---:|---:|\n");
            out.push_str(&format!(
                "| **Total** | {} ({}/{}) | {} ({}/{}) | {} ({}/{}) |\n",
                percent(t.lines),
                t.lines.hit,
                t.lines.found,
                percent(t.functions),
                t.functions.hit,
                t.functions.found,
                percent(t.branches),
                t.branches.hit,
                t.branches.found
            ));
            for file in &overview.files {
                let m = &file.metrics;
                let name = file
                    .path
                    .strip_prefix(&overview.common_prefix)
                    .unwrap_or(&file.path)
                    .replace('|', "\\|");
                out.push_str(&format!(
                    "| `{name}` | {} | {} | {} |\n",
                    percent(m.lines),
                    percent(m.functions),
                    percent(m.branches)
                ));
            }
        }
    }
    if out.is_empty() {
        return Err(PluginError::new("lcov.empty"));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "analysis_test.rs"]
mod tests;
