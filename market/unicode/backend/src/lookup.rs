//! 查找字符：按码位（U+1F600、0x4E2D、&#128512;、\u{1F600}）、字符本身或名称关键字搜索，
//! 并给出各种编程语言与 HTML、URL 中的写法。

use serde::Serialize;

use crate::analyze::{self, Flag};

const MAX_RESULTS: usize = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Escapes {
    pub rust: String,
    pub javascript: String,
    pub python: String,
    pub java: String,
    pub html: String,
    pub css: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub char: String,
    pub code: String,
    pub name: Option<String>,
    pub category: &'static str,
    pub script: String,
    pub decimal: u32,
    pub utf8: String,
    pub utf16: String,
    pub flags: Vec<Flag>,
    pub escapes: Escapes,
}

pub fn escapes(c: char) -> Escapes {
    let value = c as u32;
    let mut units = [0u16; 2];
    let utf16: Vec<String> = c
        .encode_utf16(&mut units)
        .iter()
        .map(|u| format!("\\u{u:04X}"))
        .collect();
    let mut bytes = [0u8; 4];
    let url: String = c
        .encode_utf8(&mut bytes)
        .bytes()
        .map(|b| format!("%{b:02X}"))
        .collect();
    Escapes {
        rust: format!("\\u{{{value:X}}}"),
        javascript: if value > 0xffff {
            format!("\\u{{{value:X}}}")
        } else {
            format!("\\u{value:04X}")
        },
        python: if value > 0xffff {
            format!("\\U{value:08X}")
        } else {
            format!("\\u{value:04X}")
        },
        java: utf16.concat(),
        html: format!("&#x{value:X};"),
        css: format!("\\{value:X}"),
        url,
    }
}

pub fn describe(c: char) -> Found {
    use unicode_script::UnicodeScript;
    Found {
        char: c.to_string(),
        code: analyze::code(c),
        name: analyze::name(c),
        category: analyze::category(c),
        script: c.script().full_name().to_owned(),
        decimal: c as u32,
        utf8: analyze::utf8(c),
        utf16: analyze::utf16(c),
        flags: analyze::flags(c).0,
        escapes: escapes(c),
    }
}

/// 解析码位写法
fn parse_code(query: &str) -> Option<char> {
    let q = query.trim();
    let hex = q
        .strip_prefix("U+")
        .or_else(|| q.strip_prefix("u+"))
        .or_else(|| q.strip_prefix("0x"))
        .or_else(|| q.strip_prefix("0X"))
        .or_else(|| q.strip_prefix("&#x").and_then(|s| s.strip_suffix(';')))
        .or_else(|| q.strip_prefix("\\u{").and_then(|s| s.strip_suffix('}')))
        .or_else(|| q.strip_prefix("\\u"))
        .or_else(|| q.strip_prefix("\\U"));
    if let Some(hex) = hex {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(decimal) = q.strip_prefix("&#").and_then(|s| s.strip_suffix(';')) {
        return decimal.parse().ok().and_then(char::from_u32);
    }
    None
}

pub fn lookup(query: &str) -> Vec<Found> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Some(c) = parse_code(trimmed) {
        return vec![describe(c)];
    }
    // 输入的就是几个字符（不是单词）：逐个描述
    let mut chars = query.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return vec![describe(c)];
    }
    if query.chars().count() <= 8 && !query.is_ascii() {
        return query.chars().map(describe).collect();
    }
    // 名称完全相同的字符排在最前，其后是名称包含所有关键字的字符
    let exact = unicode_names2::character(trimmed);
    let words: Vec<String> = trimmed
        .to_uppercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut found: Vec<Found> = exact.map(describe).into_iter().collect();
    for value in 0..=0x10ffffu32 {
        let Some(c) = char::from_u32(value) else {
            continue;
        };
        if Some(c) == exact {
            continue;
        }
        let Some(name) = unicode_names2::name(c) else {
            continue;
        };
        let name = name.to_string();
        if words.iter().all(|w| name.contains(w.as_str())) {
            found.push(describe(c));
            if found.len() >= MAX_RESULTS {
                break;
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "lookup_test.rs"]
mod tests;
