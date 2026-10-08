//! 转码与换行符处理：按来源编码解码，统一换行，再编码成目标编码。

use std::borrow::Cow;

use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

/// 可选的编码，按 encoding_rs 的名称
pub const ENCODINGS: &[&str] = &[
    "UTF-8",
    "UTF-16LE",
    "UTF-16BE",
    "gb18030",
    "GBK",
    "Big5",
    "Shift_JIS",
    "EUC-JP",
    "EUC-KR",
    "windows-1252",
    "windows-1251",
    "KOI8-R",
    "ISO-8859-2",
];

pub fn encoding(name: &str) -> PluginResult<&'static Encoding> {
    Encoding::for_label(name.as_bytes())
        .filter(|e| ENCODINGS.contains(&e.name()))
        .ok_or_else(|| PluginError::new("encoding.unsupported").with("encoding", name))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LineEndings {
    None,
    Lf,
    Crlf,
    Cr,
    Mixed,
}

pub fn line_endings(text: &str) -> LineEndings {
    let bytes = text.as_bytes();
    let (mut lf, mut crlf, mut cr) = (0usize, 0usize, 0usize);
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                crlf += 1;
                i += 1;
            }
            b'\r' => cr += 1,
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }
    match (lf > 0, crlf > 0, cr > 0) {
        (false, false, false) => LineEndings::None,
        (true, false, false) => LineEndings::Lf,
        (false, true, false) => LineEndings::Crlf,
        (false, false, true) => LineEndings::Cr,
        _ => LineEndings::Mixed,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NewLine {
    #[default]
    Keep,
    Lf,
    Crlf,
}

fn convert_newlines(text: &str, newline: NewLine) -> Cow<'_, str> {
    let target = match newline {
        NewLine::Keep => return Cow::Borrowed(text),
        NewLine::Lf => "\n",
        NewLine::Crlf => "\r\n",
    };
    if !text.contains('\r') && newline == NewLine::Lf {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + text.len() / 32);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str(target);
            }
            '\n' => out.push_str(target),
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

fn line_of(text: &str, byte: usize) -> usize {
    text[..byte].matches('\n').count() + 1
}

/// 按编码解码，去掉与该编码一致的 BOM；遇到无法识别的字节时报出所在行
pub fn decode<'a>(bytes: &'a [u8], from: &'static Encoding) -> PluginResult<Cow<'a, str>> {
    let (text, had_errors) = from.decode_with_bom_removal(bytes);
    if had_errors {
        let at = text.find('\u{fffd}').unwrap_or(0);
        return Err(PluginError::new("encoding.malformed")
            .with("encoding", from.name())
            .with("line", line_of(&text, at)));
    }
    Ok(text)
}

/// 预览用：无法识别的字节显示为替换字符
pub fn decode_lossy<'a>(bytes: &'a [u8], from: &'static Encoding) -> (Cow<'a, str>, bool) {
    from.decode_with_bom_removal(bytes)
}

/// 找出目标编码表示不了的第一个字符
pub fn first_unmappable(text: &str, to: &'static Encoding) -> Option<(char, usize)> {
    if to == UTF_8 || to == UTF_16LE || to == UTF_16BE {
        return None;
    }
    let mut line = 1;
    let mut buf = [0u8; 4];
    for c in text.chars() {
        if c == '\n' {
            line += 1;
        }
        if !c.is_ascii() && to.encode(c.encode_utf8(&mut buf)).2 {
            return Some((c, line));
        }
    }
    None
}

fn encode(text: &str, to: &'static Encoding) -> PluginResult<Vec<u8>> {
    if to == UTF_8 {
        return Ok(text.as_bytes().to_vec());
    }
    // encoding_rs 不编码 UTF-16，自己写
    if to == UTF_16LE || to == UTF_16BE {
        let mut out = Vec::with_capacity(text.len() * 2);
        for unit in text.encode_utf16() {
            out.extend_from_slice(&if to == UTF_16LE {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        return Ok(out);
    }
    let (bytes, _, unmappable) = to.encode(text);
    if unmappable {
        let (c, line) = first_unmappable(text, to).unwrap_or(('\u{fffd}', 0));
        return Err(PluginError::new("encoding.unmappable")
            .with("encoding", to.name())
            .with("char", c.to_string())
            .with("code", format!("U+{:04X}", c as u32))
            .with("line", line));
    }
    Ok(bytes.into_owned())
}

fn bom(to: &'static Encoding) -> &'static [u8] {
    if to == UTF_8 {
        b"\xef\xbb\xbf"
    } else if to == UTF_16LE {
        b"\xff\xfe"
    } else if to == UTF_16BE {
        b"\xfe\xff"
    } else {
        b""
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub encoding: &'static Encoding,
    /// 只对 UTF-8 与 UTF-16 有效
    pub bom: bool,
    pub newline: NewLine,
}

/// 转码；结果与原内容相同时返回 None
pub fn transcode(
    bytes: &[u8],
    from: &'static Encoding,
    target: Target,
) -> PluginResult<Option<Vec<u8>>> {
    let text = decode(bytes, from)?;
    let text = convert_newlines(&text, target.newline);
    let mut out = Vec::new();
    if target.bom {
        out.extend_from_slice(bom(target.encoding));
    }
    out.extend_from_slice(&encode(&text, target.encoding)?);
    Ok((out != bytes).then_some(out))
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
