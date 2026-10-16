//! SemVer 2.0.0 版本：严格解析与优先级比较（忽略构建元数据）。

use std::cmp::Ordering;

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

pub const MAX_LENGTH: usize = 256;
/// 与 npm 一致：数字部分不超过 2^53 - 1
pub const MAX_NUMBER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Identifier {
    Numeric(u64),
    Alpha(String),
}

impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Identifier::Numeric(a), Identifier::Numeric(b)) => a.cmp(b),
            // 数字标识符优先级低于字母数字标识符
            (Identifier::Numeric(_), Identifier::Alpha(_)) => Ordering::Less,
            (Identifier::Alpha(_), Identifier::Numeric(_)) => Ordering::Greater,
            (Identifier::Alpha(a), Identifier::Alpha(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Identifier::Numeric(n) => write!(f, "{n}"),
            Identifier::Alpha(s) => f.write_str(s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Vec<Identifier>,
    pub build: Vec<String>,
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Vec::new(),
            build: Vec::new(),
        }
    }

    /// 带 "-0" 预发布的版本，用作排他上界（与 npm 一致）
    pub fn floor(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            pre: vec![Identifier::Numeric(0)],
            ..Self::new(major, minor, patch)
        }
    }

    pub fn same_core(&self, other: &Version) -> bool {
        (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)
    }

    /// 按 SemVer 优先级比较：构建元数据不参与
    pub fn precedence(&self, other: &Version) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            let pre: Vec<String> = self.pre.iter().map(ToString::to_string).collect();
            write!(f, "-{}", pre.join("."))?;
        }
        if !self.build.is_empty() {
            write!(f, "+{}", self.build.join("."))?;
        }
        Ok(())
    }
}

fn error(code: &'static str, input: &str) -> PluginError {
    PluginError::new(code).with("input", input.to_owned())
}

/// 数字部分：不能为空、不能有前导零、不能超过 MAX_NUMBER
pub fn number(part: &str, input: &str) -> PluginResult<u64> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error("semver.invalid_number", input).with("part", part.to_owned()));
    }
    if part.len() > 1 && part.starts_with('0') {
        return Err(error("semver.leading_zero", input).with("part", part.to_owned()));
    }
    part.parse::<u64>()
        .ok()
        .filter(|n| *n <= MAX_NUMBER)
        .ok_or_else(|| error("semver.number_too_large", input).with("part", part.to_owned()))
}

fn identifiers(text: &str, input: &str, numeric_rules: bool) -> PluginResult<Vec<String>> {
    text.split('.')
        .map(|id| {
            let valid =
                !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
            if !valid {
                return Err(error("semver.invalid_identifier", input).with("part", id.to_owned()));
            }
            if numeric_rules
                && id.len() > 1
                && id.starts_with('0')
                && id.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(error("semver.leading_zero", input).with("part", id.to_owned()));
            }
            Ok(id.to_owned())
        })
        .collect()
}

/// 严格解析一个完整版本；允许前缀 v 或 =（与 npm 的 valid() 一致）
pub fn parse(input: &str) -> PluginResult<Version> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(error("semver.empty", input));
    }
    if trimmed.len() > MAX_LENGTH {
        return Err(error("semver.too_long", input).with("max", MAX_LENGTH));
    }
    let text = trimmed
        .trim_start_matches('=')
        .trim_start_matches(['v', 'V']);
    let (rest, build) = match text.split_once('+') {
        Some((rest, build)) => (rest, identifiers(build, input, false)?),
        None => (text, Vec::new()),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((core, pre)) => (core, identifiers(pre, input, true)?),
        None => (rest, Vec::new()),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return Err(error("semver.invalid_core", input));
    };
    Ok(Version {
        major: number(major, input)?,
        minor: number(minor, input)?,
        patch: number(patch, input)?,
        pre: pre
            .into_iter()
            .map(|id| match id.parse::<u64>() {
                Ok(n) if id.bytes().all(|b| b.is_ascii_digit()) => Identifier::Numeric(n),
                _ => Identifier::Alpha(id),
            })
            .collect(),
        build,
    })
}

#[cfg(test)]
#[path = "version_test.rs"]
mod tests;
