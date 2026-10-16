//! LCOV `.info` 解析。
//!
//! 支持的记录：TN、SF、VER、DA（可带校验和）、FN（`FN:行,名称` 与 lcov 2 的 `FN:行,结束行,名称`）、
//! FNDA、FNL / FNA（lcov 2.2 的函数别名记录）、BRDA、LF / LH / FNF / FNH / BRF / BRH、end_of_record。
//! 其他记录类型不参与统计，只按类型计数提示。
//!
//! 合并规则：同一源文件出现在多个记录（不同测试名或重复记录）时，行号、函数名、分支各自取并集，
//! 命中次数相加；分支的 `-`（所在代码块未执行）与数字合并时取数字，两边都是 `-` 时仍为 `-`。

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineCov {
    pub hits: u64,
    /// DA 记录中的源码行校验和（MD5，base64）
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Function {
    pub line: Option<u32>,
    pub end_line: Option<u32>,
    pub hits: u64,
}

/// 分支键：(行号, 代码块, 分支)
pub type BranchKey = (u32, String, String);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileCov {
    pub lines: BTreeMap<u32, LineCov>,
    pub functions: BTreeMap<String, Function>,
    /// None 表示 `-`：所在代码块从未执行
    pub branches: BTreeMap<BranchKey, Option<u64>>,
    pub tests: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub found: u64,
    pub hit: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub lines: Counts,
    pub functions: Counts,
    pub branches: Counts,
    /// 所在代码块未执行（`-`）的分支数，已计入 branches.found
    pub branches_not_executed: u64,
}

impl Metrics {
    pub fn add(&mut self, other: &Metrics) {
        self.lines.found += other.lines.found;
        self.lines.hit += other.lines.hit;
        self.functions.found += other.functions.found;
        self.functions.hit += other.functions.hit;
        self.branches.found += other.branches.found;
        self.branches.hit += other.branches.hit;
        self.branches_not_executed += other.branches_not_executed;
    }
}

impl FileCov {
    pub fn metrics(&self) -> Metrics {
        Metrics {
            lines: Counts {
                found: self.lines.len() as u64,
                hit: self.lines.values().filter(|l| l.hits > 0).count() as u64,
            },
            functions: Counts {
                found: self.functions.len() as u64,
                hit: self.functions.values().filter(|f| f.hits > 0).count() as u64,
            },
            branches: Counts {
                found: self.branches.len() as u64,
                hit: self
                    .branches
                    .values()
                    .filter(|b| b.is_some_and(|n| n > 0))
                    .count() as u64,
            },
            branches_not_executed: self.branches.values().filter(|b| b.is_none()).count() as u64,
        }
    }

    fn merge(&mut self, other: FileCov) {
        for (line, cov) in other.lines {
            let entry = self.lines.entry(line).or_default();
            entry.hits = entry.hits.saturating_add(cov.hits);
            if entry.checksum.is_none() {
                entry.checksum = cov.checksum;
            }
        }
        for (name, function) in other.functions {
            let entry = self.functions.entry(name).or_default();
            entry.hits = entry.hits.saturating_add(function.hits);
            entry.line = entry.line.or(function.line);
            entry.end_line = entry.end_line.or(function.end_line);
        }
        for (key, taken) in other.branches {
            let entry = self.branches.entry(key).or_insert(None);
            *entry = match (*entry, taken) {
                (Some(a), Some(b)) => Some(a.saturating_add(b)),
                (a, b) => a.or(b),
            };
        }
        self.tests.extend(other.tests);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// malformed / outsideRecord / unterminated / declaredMismatch / duplicateRecord
    pub kind: &'static str,
    /// 报告中的行号（从 1 开始）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 诊断条数上限，超出部分只计数
pub const MAX_DIAGNOSTICS: usize = 200;

#[derive(Debug, Default)]
pub struct Report {
    pub version: Option<String>,
    pub tests: Vec<String>,
    pub files: BTreeMap<String, FileCov>,
    pub records: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub diagnostics_dropped: usize,
    /// 不支持的记录类型 → 出现次数
    pub ignored: BTreeMap<String, usize>,
}

/// 当前记录（SF … end_of_record）
struct Record {
    path: String,
    cov: FileCov,
    start: usize,
    /// LF / LH / FNF / FNH / BRF / BRH 声明值
    declared: Vec<(&'static str, u64)>,
    /// FNL 的序号 → (起始行, 结束行)
    aliases: HashMap<String, (Option<u32>, Option<u32>)>,
}

#[derive(Default)]
pub struct Parser {
    report: Report,
    test: String,
    record: Option<Record>,
    line_no: usize,
    /// 已出现过的 (测试名, 源文件)，用于提示重复记录
    seen: BTreeSet<(String, String)>,
}

impl Parser {
    fn diagnostic(&mut self, kind: &'static str, file: Option<String>, detail: Option<String>) {
        if self.report.diagnostics.len() >= MAX_DIAGNOSTICS {
            self.report.diagnostics_dropped += 1;
            return;
        }
        self.report.diagnostics.push(Diagnostic {
            kind,
            line: Some(self.line_no),
            file,
            detail,
        });
    }

    fn malformed(&mut self, raw: &str) {
        let file = self.record.as_ref().map(|r| r.path.clone());
        self.diagnostic("malformed", file, Some(raw.chars().take(120).collect()));
    }

    pub fn feed(&mut self, raw: &str) {
        self.line_no += 1;
        let line = raw.trim_end_matches(['\r', '\n']).trim();
        if line.is_empty() || line.starts_with('#') {
            return;
        }
        if line == "end_of_record" {
            self.finish_record();
            return;
        }
        let Some((kind, value)) = line.split_once(':') else {
            self.malformed(line);
            return;
        };
        match kind {
            "TN" => {
                self.test = value.trim().to_owned();
                if !self.test.is_empty() && !self.report.tests.contains(&self.test) {
                    self.report.tests.push(self.test.clone());
                }
            }
            "VER" => self.report.version = Some(value.trim().to_owned()),
            "SF" => {
                if self.record.is_some() {
                    // 上一个记录缺少 end_of_record
                    let file = self.record.as_ref().map(|r| r.path.clone());
                    self.diagnostic("unterminated", file, None);
                    self.finish_record();
                }
                let path = value.trim().to_owned();
                if !self.seen.insert((self.test.clone(), path.clone())) {
                    self.diagnostic(
                        "duplicateRecord",
                        Some(path.clone()),
                        Some(self.test.clone()),
                    );
                }
                let mut cov = FileCov::default();
                if !self.test.is_empty() {
                    cov.tests.insert(self.test.clone());
                }
                self.record = Some(Record {
                    path,
                    cov,
                    start: self.line_no,
                    declared: Vec::new(),
                    aliases: HashMap::new(),
                });
            }
            _ => {
                if self.record.is_none() {
                    if is_known(kind) {
                        self.diagnostic("outsideRecord", None, Some(kind.to_owned()));
                    } else {
                        *self.report.ignored.entry(kind.to_owned()).or_default() += 1;
                    }
                    return;
                }
                if !self.record_entry(kind, value) {
                    if is_known(kind) {
                        self.malformed(line);
                    } else {
                        *self.report.ignored.entry(kind.to_owned()).or_default() += 1;
                    }
                }
            }
        }
    }

    /// 处理记录内的一行；无法识别或格式错误时返回 false
    fn record_entry(&mut self, kind: &str, value: &str) -> bool {
        let Some(record) = self.record.as_mut() else {
            return false;
        };
        let cov = &mut record.cov;
        let mut parts = value.split(',');
        match kind {
            "DA" => {
                let (Some(Ok(line)), Some(Ok(hits))) = (
                    parts.next().map(|p| p.trim().parse::<u32>()),
                    parts.next().map(|p| parse_hits(p.trim())),
                ) else {
                    return false;
                };
                let checksum = parts.next().map(|c| c.trim().to_owned());
                let entry = cov.lines.entry(line).or_default();
                entry.hits = entry.hits.saturating_add(hits);
                if checksum.is_some() {
                    entry.checksum = checksum;
                }
                true
            }
            "FN" => {
                let fields: Vec<&str> = value.splitn(3, ',').collect();
                let parsed = match fields.as_slice() {
                    // lcov 2：FN:起始行,结束行,名称（名称本身可能含逗号）
                    [line, end, name] if end.trim().parse::<u32>().is_ok() => {
                        Some((line.trim().parse().ok(), end.trim().parse().ok(), *name))
                    }
                    [line, rest @ ..] => {
                        let name = &value[line.len() + 1..];
                        (!rest.is_empty()).then(|| (line.trim().parse().ok(), None, name))
                    }
                    _ => None,
                };
                let Some((Some(line), end, name)) = parsed else {
                    return false;
                };
                let entry = cov.functions.entry(name.to_owned()).or_default();
                entry.line = Some(line);
                entry.end_line = end;
                true
            }
            "FNDA" => {
                let Some((hits, name)) = value.split_once(',') else {
                    return false;
                };
                let Ok(hits) = parse_hits(hits.trim()) else {
                    return false;
                };
                let entry = cov.functions.entry(name.to_owned()).or_default();
                entry.hits = entry.hits.saturating_add(hits);
                true
            }
            "FNL" => {
                let fields: Vec<&str> = value.split(',').map(str::trim).collect();
                let (Some(index), Some(Ok(line))) =
                    (fields.first(), fields.get(1).map(|l| l.parse::<u32>()))
                else {
                    return false;
                };
                let end = fields.get(2).and_then(|e| e.parse().ok());
                record
                    .aliases
                    .insert((*index).to_owned(), (Some(line), end));
                true
            }
            "FNA" => {
                let fields: Vec<&str> = value.splitn(3, ',').collect();
                let [index, hits, name] = fields.as_slice() else {
                    return false;
                };
                let Ok(hits) = parse_hits(hits.trim()) else {
                    return false;
                };
                let (line, end) = record
                    .aliases
                    .get(index.trim())
                    .copied()
                    .unwrap_or((None, None));
                let entry = record.cov.functions.entry((*name).to_owned()).or_default();
                entry.hits = entry.hits.saturating_add(hits);
                entry.line = entry.line.or(line);
                entry.end_line = entry.end_line.or(end);
                true
            }
            "BRDA" => {
                let fields: Vec<&str> = value.split(',').map(str::trim).collect();
                let [line, block, branch, taken] = fields.as_slice() else {
                    return false;
                };
                let Ok(line) = line.parse::<u32>() else {
                    return false;
                };
                let taken = match *taken {
                    "-" => None,
                    n => match parse_hits(n) {
                        Ok(n) => Some(n),
                        Err(_) => return false,
                    },
                };
                let entry = cov
                    .branches
                    .entry((line, (*block).to_owned(), (*branch).to_owned()))
                    .or_insert(None);
                *entry = match (*entry, taken) {
                    (Some(a), Some(b)) => Some(a.saturating_add(b)),
                    (a, b) => a.or(b),
                };
                true
            }
            "LF" | "LH" | "FNF" | "FNH" | "BRF" | "BRH" => {
                let Ok(n) = value.trim().parse::<u64>() else {
                    return false;
                };
                let key = match kind {
                    "LF" => "LF",
                    "LH" => "LH",
                    "FNF" => "FNF",
                    "FNH" => "FNH",
                    "BRF" => "BRF",
                    _ => "BRH",
                };
                record.declared.push((key, n));
                true
            }
            _ => false,
        }
    }

    fn finish_record(&mut self) {
        let Some(record) = self.record.take() else {
            return;
        };
        self.report.records += 1;
        let metrics = record.cov.metrics();
        for (key, declared) in &record.declared {
            let computed = match *key {
                "LF" => metrics.lines.found,
                "LH" => metrics.lines.hit,
                "FNF" => metrics.functions.found,
                "FNH" => metrics.functions.hit,
                "BRF" => metrics.branches.found,
                _ => metrics.branches.hit,
            };
            if computed != *declared {
                let saved = self.line_no;
                self.line_no = record.start;
                self.diagnostic(
                    "declaredMismatch",
                    Some(record.path.clone()),
                    Some(format!("{key}: {declared} ≠ {computed}")),
                );
                self.line_no = saved;
            }
        }
        match self.report.files.get_mut(&record.path) {
            Some(existing) => existing.merge(record.cov),
            None => {
                self.report.files.insert(record.path, record.cov);
            }
        }
    }

    pub fn finish(mut self) -> Report {
        if let Some(record) = &self.record {
            let file = Some(record.path.clone());
            self.diagnostic("unterminated", file, None);
            self.finish_record();
        }
        self.report
    }
}

fn is_known(kind: &str) -> bool {
    matches!(
        kind,
        "DA" | "FN" | "FNDA" | "FNL" | "FNA" | "BRDA" | "LF" | "LH" | "FNF" | "FNH" | "BRF" | "BRH"
    )
}

/// 命中次数；部分工具会写出负数或浮点，按 0 / 向下取整处理
fn parse_hits(text: &str) -> Result<u64, ()> {
    if let Ok(n) = text.parse::<u64>() {
        return Ok(n);
    }
    if let Ok(n) = text.parse::<i64>() {
        return Ok(n.max(0) as u64);
    }
    text.parse::<f64>()
        .ok()
        .filter(|f| f.is_finite())
        .map(|f| f.max(0.0) as u64)
        .ok_or(())
}

pub fn parse_text(text: &str) -> Report {
    let mut parser = Parser::default();
    for line in text.lines() {
        parser.feed(line);
    }
    parser.finish()
}

#[cfg(test)]
#[path = "lcov_test.rs"]
mod tests;
