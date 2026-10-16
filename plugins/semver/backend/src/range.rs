//! npm（node-semver）范围语义：把 ^ ~ 通配、连字符范围展开为比较器集合，并判断版本是否命中。

use std::cmp::Ordering;

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::version::{self, Version};

pub const MAX_RANGE: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Op {
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "=")]
    Eq,
}

impl Op {
    fn symbol(self) -> &'static str {
        match self {
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Eq => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comparator {
    pub op: Op,
    pub version: Version,
}

impl Comparator {
    fn new(op: Op, version: Version) -> Self {
        Self { op, version }
    }

    pub fn test(&self, v: &Version) -> bool {
        let ord = v.precedence(&self.version);
        match self.op {
            Op::Lt => ord == Ordering::Less,
            Op::Le => ord != Ordering::Greater,
            Op::Gt => ord == Ordering::Greater,
            Op::Ge => ord != Ordering::Less,
            Op::Eq => ord == Ordering::Equal,
        }
    }
}

impl std::fmt::Display for Comparator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.op.symbol(), self.version)
    }
}

/// 一组比较器（交集）；空集合表示匹配任意版本（*）
pub type Set = Vec<Comparator>;

#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub sets: Vec<Set>,
}

/// 可能不完整的版本：缺失或 x / X / * 的部分为 None
#[derive(Debug, Clone, PartialEq)]
struct Partial {
    major: Option<u64>,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: Vec<version::Identifier>,
}

impl Partial {
    fn full(&self) -> Option<Version> {
        Some(Version {
            pre: self.pre.clone(),
            ..Version::new(self.major?, self.minor?, self.patch?)
        })
    }

    fn lower(&self) -> Version {
        Version {
            pre: if self.patch.is_some() {
                self.pre.clone()
            } else {
                Vec::new()
            },
            ..Version::new(
                self.major.unwrap_or(0),
                self.minor.unwrap_or(0),
                self.patch.unwrap_or(0),
            )
        }
    }
}

fn range_error(code: &'static str, token: &str, column: usize) -> PluginError {
    PluginError::new(code)
        .with("token", token.to_owned())
        .with("column", column)
}

fn is_wild(part: &str) -> bool {
    matches!(part, "x" | "X" | "*")
}

fn partial(text: &str, column: usize) -> PluginResult<Partial> {
    let invalid = || range_error("semver.invalid_range", text, column);
    let text = text.trim_start_matches('=').trim_start_matches(['v', 'V']);
    if text.is_empty() || is_wild(text) {
        return Ok(Partial {
            major: None,
            minor: None,
            patch: None,
            pre: Vec::new(),
        });
    }
    let text = text.split_once('+').map_or(text, |(core, _)| core);
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (text, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() > 3 {
        return Err(invalid());
    }
    let mut numbers = [None, None, None];
    let mut wild = false;
    for (index, part) in parts.iter().enumerate() {
        if is_wild(part) {
            wild = true;
            continue;
        }
        if wild {
            // 1.x.3 这类通配之后又出现数字的写法无效
            return Err(invalid());
        }
        numbers[index] = Some(version::number(part, text).map_err(|e| e.with("column", column))?);
    }
    let pre = match pre {
        Some(pre) if numbers.iter().all(Option::is_some) => {
            version::parse(&format!("0.0.0-{pre}"))
                .map_err(|e| e.with("column", column))?
                .pre
        }
        Some(_) => return Err(invalid()),
        None => Vec::new(),
    };
    Ok(Partial {
        major: numbers[0],
        minor: numbers[1],
        patch: numbers[2],
        pre,
    })
}

fn ge(v: Version) -> Comparator {
    Comparator::new(Op::Ge, v)
}

fn lt(v: Version) -> Comparator {
    Comparator::new(Op::Lt, v)
}

/// x 范围：1 → >=1.0.0 <2.0.0-0，1.2 → >=1.2.0 <1.3.0-0，* → 任意
fn x_range(p: &Partial) -> Set {
    match (p.major, p.minor, p.patch) {
        (None, ..) => Vec::new(),
        (Some(ma), None, _) => vec![ge(Version::new(ma, 0, 0)), lt(Version::floor(ma + 1, 0, 0))],
        (Some(ma), Some(mi), None) => vec![
            ge(Version::new(ma, mi, 0)),
            lt(Version::floor(ma, mi + 1, 0)),
        ],
        (Some(_), Some(_), Some(_)) => vec![Comparator::new(Op::Eq, p.full().expect("complete"))],
    }
}

fn tilde(p: &Partial) -> Set {
    match (p.major, p.minor, p.patch) {
        (None, ..) => Vec::new(),
        (Some(ma), None, _) => vec![ge(Version::new(ma, 0, 0)), lt(Version::floor(ma + 1, 0, 0))],
        (Some(ma), Some(mi), _) => vec![ge(p.lower()), lt(Version::floor(ma, mi + 1, 0))],
    }
}

fn caret(p: &Partial) -> Set {
    match (p.major, p.minor, p.patch) {
        (None, ..) => Vec::new(),
        (Some(ma), None, _) => vec![ge(Version::new(ma, 0, 0)), lt(Version::floor(ma + 1, 0, 0))],
        (Some(0), Some(mi), None) => {
            vec![ge(Version::new(0, mi, 0)), lt(Version::floor(0, mi + 1, 0))]
        }
        (Some(ma), Some(mi), None) => vec![
            ge(Version::new(ma, mi, 0)),
            lt(Version::floor(ma + 1, 0, 0)),
        ],
        (Some(0), Some(0), Some(pa)) => vec![ge(p.lower()), lt(Version::floor(0, 0, pa + 1))],
        (Some(0), Some(mi), Some(_)) => vec![ge(p.lower()), lt(Version::floor(0, mi + 1, 0))],
        (Some(ma), Some(_), Some(_)) => vec![ge(p.lower()), lt(Version::floor(ma + 1, 0, 0))],
    }
}

/// 不完整版本上的普通比较运算符
fn primitive(op: Op, p: &Partial) -> Set {
    let Some(ma) = p.major else {
        // >=* 与 <=* 匹配任意版本；>* 与 <* 什么也不匹配
        return match op {
            Op::Ge | Op::Le | Op::Eq => Vec::new(),
            Op::Gt | Op::Lt => vec![lt(Version::floor(0, 0, 0))],
        };
    };
    if let Some(full) = p.full() {
        return vec![Comparator::new(op, full)];
    }
    let (mi, partial_minor) = (p.minor, p.minor.is_none());
    match op {
        Op::Gt => vec![ge(if partial_minor {
            Version::new(ma + 1, 0, 0)
        } else {
            Version::new(ma, mi.unwrap_or(0) + 1, 0)
        })],
        Op::Ge => vec![ge(p.lower())],
        Op::Lt => vec![lt(Version::floor(ma, mi.unwrap_or(0), 0))],
        Op::Le => vec![lt(if partial_minor {
            Version::floor(ma + 1, 0, 0)
        } else {
            Version::floor(ma, mi.unwrap_or(0) + 1, 0)
        })],
        Op::Eq => x_range(p),
    }
}

/// 连字符范围：1.2 - 2.3 → >=1.2.0 <2.4.0-0
fn hyphen(from: &Partial, to: &Partial) -> Set {
    let mut set = Vec::new();
    if from.major.is_some() {
        set.push(ge(from.lower()));
    }
    match (to.major, to.minor, to.patch) {
        (None, ..) => {}
        (Some(ma), None, _) => set.push(lt(Version::floor(ma + 1, 0, 0))),
        (Some(ma), Some(mi), None) => set.push(lt(Version::floor(ma, mi + 1, 0))),
        _ => set.push(Comparator::new(Op::Le, to.full().expect("complete"))),
    }
    set
}

/// 拆出带位置（1 起始列号）的空白分隔片段
fn tokens(text: &str, base: usize) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (index, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                out.push((base + text[..s].chars().count() + 1, &text[s..index]));
                start = None;
            }
            (false, None) => start = Some(index),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((base + text[..s].chars().count() + 1, &text[s..]));
    }
    out
}

/// 把 ">= 1.2" 这种运算符与版本之间有空格的写法合并
fn merge_operators(raw: Vec<(usize, &str)>) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut pending: Option<(usize, String)> = None;
    for (column, token) in raw {
        if let Some((col, op)) = pending.take() {
            out.push((col, format!("{op}{token}")));
            continue;
        }
        if matches!(token, "<" | "<=" | ">" | ">=" | "=" | "~" | "~>" | "^") {
            pending = Some((column, token.to_owned()));
        } else {
            out.push((column, token.to_owned()));
        }
    }
    if let Some((column, op)) = pending {
        out.push((column, op));
    }
    out
}

fn comparator_set(text: &str, base: usize) -> PluginResult<Set> {
    let raw = tokens(text, base);
    // 连字符范围：a - b
    if raw.len() == 3 && raw[1].1 == "-" {
        let from = partial(raw[0].1, raw[0].0)?;
        let to = partial(raw[2].1, raw[2].0)?;
        return Ok(hyphen(&from, &to));
    }
    let mut set = Vec::new();
    for (column, token) in merge_operators(raw) {
        let (op, rest) = [
            ("~>", None),
            ("<=", Some(Op::Le)),
            (">=", Some(Op::Ge)),
            ("~", None),
            ("^", None),
            ("<", Some(Op::Lt)),
            (">", Some(Op::Gt)),
            ("=", Some(Op::Eq)),
        ]
        .into_iter()
        .find_map(|(prefix, op)| token.strip_prefix(prefix).map(|rest| ((prefix, op), rest)))
        .unwrap_or((("", Some(Op::Eq)), token.as_str()));
        if rest.is_empty() && !op.0.is_empty() {
            return Err(range_error("semver.invalid_range", &token, column));
        }
        let p = partial(rest, column)?;
        set.extend(match op {
            ("~" | "~>", _) => tilde(&p),
            ("^", _) => caret(&p),
            ("", _) => x_range(&p),
            (_, Some(op)) => primitive(op, &p),
            _ => unreachable!("every prefix maps to a rule"),
        });
    }
    Ok(set)
}

pub fn parse(text: &str) -> PluginResult<Range> {
    if text.len() > MAX_RANGE {
        return Err(PluginError::new("semver.range_too_long").with("max", MAX_RANGE));
    }
    let mut sets = Vec::new();
    let mut offset = 0;
    for part in text.split("||") {
        let base = text[..offset].chars().count();
        sets.push(comparator_set(part, base)?);
        offset += part.len() + 2;
    }
    Ok(Range { sets })
}

/// 版本在某个集合上的判断结果；失败时给出原因
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome {
    Match,
    /// 不满足该比较器
    Failed {
        comparator: String,
    },
    /// 预发布版本只有在同一 major.minor.patch 的比较器带预发布时才会命中
    Prerelease,
}

pub fn test_set(set: &Set, v: &Version, include_prerelease: bool) -> Outcome {
    if let Some(failed) = set.iter().find(|c| !c.test(v)) {
        return Outcome::Failed {
            comparator: failed.to_string(),
        };
    }
    if !v.pre.is_empty() && !include_prerelease {
        let allowed = set
            .iter()
            .any(|c| !c.version.pre.is_empty() && c.version.same_core(v));
        if !allowed {
            return Outcome::Prerelease;
        }
    }
    Outcome::Match
}

impl Range {
    /// 命中任一集合即命中；否则返回第一个集合的失败原因
    pub fn test(&self, v: &Version, include_prerelease: bool) -> Outcome {
        let mut first = None;
        for set in &self.sets {
            match test_set(set, v, include_prerelease) {
                Outcome::Match => return Outcome::Match,
                other => {
                    first.get_or_insert(other);
                }
            }
        }
        first.unwrap_or(Outcome::Match)
    }

    /// 标准化后的比较集合文本
    pub fn normalized(&self) -> Vec<String> {
        self.sets
            .iter()
            .map(|set| {
                if set.is_empty() {
                    "*".to_owned()
                } else {
                    set.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "range_test.rs"]
mod tests;
