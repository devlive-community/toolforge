//! .env 文档模型：解析为可编辑的行序列，序列化写回；
//! 注释、空行、变量顺序与引号风格全程保留，无法识别的行原样透传。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 引号风格；None 表示值不加引号
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Quote {
    #[default]
    None,
    Single,
    Double,
}

/// 文档中的一行；UI 编辑后原样传回
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Line {
    Blank,
    /// 整行注释（含行首 #）
    #[serde(rename_all = "camelCase")]
    Comment {
        text: String,
    },
    /// 无法识别的行，原样透传
    #[serde(rename_all = "camelCase")]
    Passthrough {
        text: String,
    },
    /// 一个变量；value 是转义后的单行文本（\n、\"、\\ 转义）
    #[serde(rename_all = "camelCase")]
    Entry {
        r#export: bool,
        key: String,
        value: String,
        quote: Quote,
        /// 行内注释（# 之后的文本）
        inline_comment: Option<String>,
        /// 值在源文件中是跨行的双引号块
        multiline: bool,
    },
}

/// 文件大小上限（.env 超过 1 MB 不正常）
pub const MAX_FILE: u64 = 1024 * 1024;

/// KEY 语法：字母或下划线开头，其后为字母、数字、下划线或点
pub fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// 找到下一个未转义的双引号位置（跳过 \" 与 \\ 转义对）
fn find_closing_quote(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// 还原双引号值的转义：\n、\r、\"、\\；未知转义按字面保留
pub fn unescape_double(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 反向转义；keep_newlines 为真时保留真实换行（跨行块），否则转为 \n 字面
pub fn escape_double(value: &str, keep_newlines: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' if keep_newlines => out.push('\n'),
            '\n' => out.push_str("\\n"),
            '\r' if keep_newlines => out.push('\r'),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// open 时传给 UI 的值形式：单行显示转义
pub fn escape_display(value: &str) -> String {
    escape_double(value, false)
}

/// 拆出无引号值之后的行内注释：第一个"空白 + #"处分割
fn split_inline_comment(rest: &str) -> (&str, Option<String>) {
    let bytes = rest.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let comment = rest[i + 1..].trim().to_owned();
            let value = rest[..i].trim_end();
            let comment = if comment.is_empty() {
                None
            } else {
                Some(comment)
            };
            return (value, comment);
        }
    }
    (rest.trim_end(), None)
}

/// 解析双引号值（可能跨行）；返回值、行内注释、结束行索引（含）。
/// 未闭合返回 None，由调用方按透传处理。
#[allow(clippy::type_complexity)]
fn parse_double_value(
    first: &str,
    raw: &[&str],
    start: usize,
) -> Option<(String, Option<String>, usize)> {
    let mut value = String::new();
    let mut index = start;
    let mut cursor = first;
    loop {
        match find_closing_quote(cursor) {
            Some(pos) => {
                value.push_str(&cursor[..pos]);
                let after = &cursor[pos + 1..];
                let (_, comment) = split_inline_comment(after);
                let tail = after.trim();
                // 闭合引号后除注释外还有内容，无法安全解释
                if !tail.is_empty() && comment.is_none() {
                    return None;
                }
                return Some((unescape_double(&value), comment, index));
            }
            None => {
                value.push_str(cursor);
                if index + 1 >= raw.len() {
                    return None;
                }
                index += 1;
                let next = raw[index].strip_suffix('\r').unwrap_or(raw[index]);
                value.push('\n');
                cursor = next;
            }
        }
    }
}

/// 解析整个文档；尾行保留（以换行结尾时末尾补一个空行）
pub fn parse(text: &str) -> Vec<Line> {
    let raw: Vec<&str> = text.split('\n').collect();
    // 文本以换行结尾时，split 产生的末尾空段是切分伪影而不是空行
    let bound = if text.ends_with('\n') {
        raw.len() - 1
    } else {
        raw.len()
    };
    let mut lines = Vec::with_capacity(bound);
    let mut index = 0;
    while index < bound {
        let line = raw[index].strip_suffix('\r').unwrap_or(raw[index]);
        let trimmed = line.trim();
        if line.is_empty() {
            lines.push(Line::Blank);
            index += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            lines.push(Line::Comment {
                text: line.to_owned(),
            });
            index += 1;
            continue;
        }

        let (export, rest) = match trimmed.strip_prefix("export") {
            Some(rest) if rest.starts_with(char::is_whitespace) => (true, rest.trim_start()),
            _ => (false, trimmed),
        };

        enum Parsed {
            /// 成功解析的行与它占用的末行索引
            Entry(Line, usize),
            /// 双引号未闭合到文件末尾
            UnterminatedDouble,
            /// 无法识别，单行透传
            Passthrough,
        }

        let parsed = match rest.find('=') {
            Some(eq) if eq > 0 => {
                let key = rest[..eq].trim_end();
                if !is_valid_key(key) {
                    Parsed::Passthrough
                } else {
                    let entry = |value: String,
                                 quote: Quote,
                                 comment: Option<String>,
                                 multiline: bool,
                                 end: usize| {
                        Parsed::Entry(
                            Line::Entry {
                                export,
                                key: key.to_owned(),
                                value,
                                quote,
                                inline_comment: comment,
                                multiline,
                            },
                            end,
                        )
                    };
                    let value = rest[eq + 1..].trim_start();
                    if let Some(inner) = value.strip_prefix('"') {
                        match parse_double_value(inner, &raw, index) {
                            Some((value, comment, end)) => {
                                entry(value, Quote::Double, comment, end > index, end)
                            }
                            None => Parsed::UnterminatedDouble,
                        }
                    } else if let Some(inner) = value.strip_prefix('\'') {
                        match inner.find('\'') {
                            Some(pos) => {
                                let after = &inner[pos + 1..];
                                let (_, comment) = split_inline_comment(after);
                                let tail = after.trim();
                                if tail.is_empty() || comment.is_some() {
                                    entry(
                                        inner[..pos].to_owned(),
                                        Quote::Single,
                                        comment,
                                        false,
                                        index,
                                    )
                                } else {
                                    Parsed::Passthrough
                                }
                            }
                            None => Parsed::Passthrough,
                        }
                    } else {
                        let (value, comment) = split_inline_comment(value);
                        entry(value.to_owned(), Quote::None, comment, false, index)
                    }
                }
            }
            _ => Parsed::Passthrough,
        };

        match parsed {
            Parsed::Entry(line, end) => {
                lines.push(line);
                index = end + 1;
            }
            // 未闭合的双引号块：从当前行起到文件末尾全部透传，避免破坏原文
            Parsed::UnterminatedDouble => {
                let text = raw[index..bound]
                    .iter()
                    .map(|l| l.strip_suffix('\r').unwrap_or(l))
                    .collect::<Vec<_>>()
                    .join("\n");
                lines.push(Line::Passthrough { text });
                break;
            }
            Parsed::Passthrough => {
                lines.push(Line::Passthrough {
                    text: line.to_owned(),
                });
                index += 1;
            }
        }
    }
    lines
}

fn push_entry(out: &mut String, line: &Line) {
    let Line::Entry {
        export,
        key,
        value,
        quote,
        inline_comment,
        multiline,
    } = line
    else {
        return;
    };
    if *export {
        out.push_str("export ");
    }
    out.push_str(key);
    out.push('=');
    match quote {
        Quote::Double => {
            out.push('"');
            out.push_str(&escape_double(value, *multiline));
            out.push('"');
        }
        Quote::Single => {
            // 单引号没有转义机制：值含单引号或换行时升级为双引号
            if value.contains('\'') || value.contains('\n') {
                out.push('"');
                out.push_str(&escape_double(value, value.contains('\n')));
                out.push('"');
            } else {
                out.push('\'');
                out.push_str(value);
                out.push('\'');
            }
        }
        Quote::None => {
            let needs_quotes = !value.is_empty()
                && (value != value.trim()
                    || value
                        .chars()
                        .any(|c| c.is_whitespace() || c == '#' || c == '"' || c == '\''));
            if needs_quotes {
                out.push('"');
                out.push_str(&escape_double(value, false));
                out.push('"');
            } else {
                out.push_str(value);
            }
        }
    }
    if let Some(comment) = inline_comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        out.push_str(" # ");
        out.push_str(comment);
    }
    out.push('\n');
}

/// 序列化；每行以 \n 结尾
pub fn serialize(lines: &[Line]) -> String {
    let mut out = String::new();
    for line in lines {
        match line {
            Line::Blank => out.push('\n'),
            Line::Comment { text } | Line::Passthrough { text } => {
                out.push_str(text);
                out.push('\n');
            }
            entry @ Line::Entry { .. } => push_entry(&mut out, entry),
        }
    }
    out
}

/// 生成 .env.example：变量保留、值清空、引号去掉，注释与结构不动
pub fn to_example_text(lines: &[Line]) -> String {
    let example: Vec<Line> = lines
        .iter()
        .map(|line| match line {
            Line::Entry {
                export,
                key,
                inline_comment,
                ..
            } => Line::Entry {
                export: *export,
                key: key.clone(),
                value: String::new(),
                quote: Quote::None,
                inline_comment: inline_comment.clone(),
                multiline: false,
            },
            other => other.clone(),
        })
        .collect();
    serialize(&example)
}

/// 保存前校验：key 语法与重复
pub fn validate(lines: &[Line]) -> PluginResult<()> {
    let mut seen: HashSet<&str> = HashSet::new();
    for line in lines {
        if let Line::Entry { key, .. } = line {
            if !is_valid_key(key) {
                return Err(PluginError::new("env.invalid_key").with("key", key.as_str()));
            }
            if !seen.insert(key.as_str()) {
                return Err(PluginError::new("env.duplicate_key").with("key", key.as_str()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "document_test.rs"]
mod tests;
