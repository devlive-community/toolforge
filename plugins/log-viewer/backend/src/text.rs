//! 编码识别、行解码与匹配。

use std::borrow::Cow;

use encoding_rs::{Encoding, GB18030, UTF_8};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::level;

/// 识别编码：UTF-16 不支持；不是合法 UTF-8 且能按 GB18030 无错解码时使用 GB18030
pub fn detect_encoding(sample: &[u8]) -> PluginResult<&'static Encoding> {
    if sample.starts_with(&[0xff, 0xfe]) || sample.starts_with(&[0xfe, 0xff]) {
        return Err(PluginError::new("log.utf16_unsupported"));
    }
    match std::str::from_utf8(sample) {
        Ok(_) => Ok(UTF_8),
        // 样本在多字节字符中间截断不算错误
        Err(err) if err.error_len().is_none() => Ok(UTF_8),
        Err(_) => {
            let (_, had_errors) = GB18030.decode_without_bom_handling(sample);
            Ok(if had_errors { UTF_8 } else { GB18030 })
        }
    }
}

/// 解码一行：去掉 BOM 与 ANSI 控制序列
pub fn decode<'a>(bytes: &'a [u8], encoding: &'static Encoding) -> Cow<'a, str> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let decoded: Cow<'a, str> = match level::strip_ansi(bytes) {
        Cow::Borrowed(raw) => {
            if encoding == UTF_8 {
                String::from_utf8_lossy(raw)
            } else {
                encoding.decode_without_bom_handling(raw).0
            }
        }
        Cow::Owned(raw) => Cow::Owned(if encoding == UTF_8 {
            String::from_utf8_lossy(&raw).into_owned()
        } else {
            encoding.decode_without_bom_handling(&raw).0.into_owned()
        }),
    };
    decoded
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    /// 要显示的级别（none / trace / debug / info / warn / error）；为空表示全部
    #[serde(default)]
    pub levels: Vec<String>,
}

pub struct Matcher {
    pattern: Option<Regex>,
    /// 按级别编号索引，true 表示显示
    levels: [bool; 6],
}

impl Matcher {
    pub fn new(filter: &Filter) -> PluginResult<Self> {
        let pattern = if filter.query.is_empty() {
            None
        } else {
            let source = if filter.regex {
                filter.query.clone()
            } else {
                regex::escape(&filter.query)
            };
            Some(
                RegexBuilder::new(&source)
                    .case_insensitive(!filter.case_sensitive)
                    .size_limit(32 * 1024 * 1024)
                    .build()
                    .map_err(|e| {
                        PluginError::new("log.invalid_regex").with("detail", e.to_string())
                    })?,
            )
        };
        let mut levels = [filter.levels.is_empty(); 6];
        for name in &filter.levels {
            match level::NAMES.iter().position(|n| n == name) {
                Some(index) => levels[index] = true,
                None => {
                    return Err(PluginError::new("log.invalid_level").with("level", name.clone()));
                }
            }
        }
        Ok(Self { pattern, levels })
    }

    pub fn is_empty(&self) -> bool {
        self.pattern.is_none() && self.levels.iter().all(|l| *l)
    }

    pub fn has_pattern(&self) -> bool {
        self.pattern.is_some()
    }

    pub fn accepts_level(&self, level: u8) -> bool {
        self.levels[level as usize]
    }

    pub fn matches(&self, text: &str) -> bool {
        self.pattern.as_ref().is_none_or(|p| p.is_match(text))
    }

    /// 匹配位置，按 UTF-16 下标（前端字符串的下标）返回，最多 50 处
    pub fn marks(&self, text: &str) -> Vec<[usize; 2]> {
        let Some(pattern) = &self.pattern else {
            return Vec::new();
        };
        let utf16 = |byte: usize| text[..byte].encode_utf16().count();
        pattern
            .find_iter(text)
            .filter(|m| !m.is_empty())
            .take(50)
            .map(|m| [utf16(m.start()), utf16(m.end())])
            .collect()
    }
}

/// 截断到最多 max 个字符
pub fn truncate(text: &str, max: usize) -> (&str, bool) {
    match text.char_indices().nth(max) {
        Some((end, _)) => (&text[..end], true),
        None => (text, false),
    }
}

/// 行内的 JSON 对象（整行或第一个 { 到最后一个 }），格式化后返回
pub fn pretty_json(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

#[cfg(test)]
#[path = "text_test.rs"]
mod tests;
