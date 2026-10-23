//! 编码识别、行解码与匹配。

use std::borrow::Cow;

use encoding_rs::{Encoding, GB18030, UTF_8};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::ansi::{self, Color, Style};
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

/// 带样式的区间，按 UTF-16 下标（前端字符串的下标）表示
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub style: Style,
}

/// 解码一行并保留 ANSI 样式：逐段解码，记录每段在结果中的 UTF-16 区间
pub fn decode_styled(bytes: &[u8], encoding: &'static Encoding) -> (String, Vec<Span>) {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if !bytes.contains(&0x1b) {
        return (decode(bytes, encoding).into_owned(), Vec::new());
    }
    let mut text = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut offset = 0;
    for run in ansi::runs(bytes) {
        let piece = decode(&bytes[run.start..run.end], encoding);
        let len = piece.encode_utf16().count();
        text.push_str(&piece);
        if !run.style.is_plain() {
            match spans.last_mut() {
                Some(last) if last.end == offset && last.style == run.style => last.end += len,
                _ => spans.push(Span {
                    start: offset,
                    end: offset + len,
                    style: run.style,
                }),
            }
        }
        offset += len;
    }
    (text, spans)
}

/// 只保留落在前 len 个 UTF-16 单元内的样式（文本被截断时）
pub fn clip_spans(spans: Vec<Span>, len: usize) -> Vec<Span> {
    spans
        .into_iter()
        .filter(|s| s.start < len)
        .map(|mut s| {
            s.end = s.end.min(len);
            s
        })
        .collect()
}

/// 终端基本色在导出 HTML 中使用的颜色（常见终端的默认配色）
const BASIC: [&str; 16] = [
    "#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
    "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
];

fn css_color(color: &Color) -> &str {
    match color {
        Color::Basic(index) => BASIC[(*index as usize).min(15)],
        Color::Rgb(hex) => hex,
    }
}

fn escape_html(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

fn css(style: &Style) -> String {
    let (mut fg, mut bg) = (
        style.fg.as_ref().map(css_color),
        style.bg.as_ref().map(css_color),
    );
    if style.inverse {
        (fg, bg) = (bg.or(Some("#1e1e1e")), fg.or(Some("#d4d4d4")));
    }
    let mut rules = Vec::new();
    if let Some(fg) = fg {
        rules.push(format!("color:{fg}"));
    }
    if let Some(bg) = bg {
        rules.push(format!("background:{bg}"));
    }
    if style.bold {
        rules.push("font-weight:bold".into());
    }
    if style.dim {
        rules.push("opacity:.7".into());
    }
    if style.italic {
        rules.push("font-style:italic".into());
    }
    match (style.underline, style.strike) {
        (true, true) => rules.push("text-decoration:underline line-through".into()),
        (true, false) => rules.push("text-decoration:underline".into()),
        (false, true) => rules.push("text-decoration:line-through".into()),
        _ => {}
    }
    rules.join(";")
}

/// 把带样式的一行渲染为 HTML（样式写在行内，便于粘贴到文档）
pub fn line_html(text: &str, spans: &[Span], out: &mut String) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let slice = |from: usize, to: usize| String::from_utf16_lossy(&units[from..to]);
    let mut cursor = 0;
    for span in spans {
        if span.start > cursor {
            escape_html(&slice(cursor, span.start), out);
        }
        out.push_str(&format!("<span style=\"{}\">", css(&span.style)));
        escape_html(&slice(span.start, span.end), out);
        out.push_str("</span>");
        cursor = span.end;
    }
    if cursor < units.len() {
        escape_html(&slice(cursor, units.len()), out);
    }
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
