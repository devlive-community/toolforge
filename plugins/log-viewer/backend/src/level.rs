//! 日志级别识别：只看每行开头的一小段，兼容常见的文本、logfmt 与 JSON 日志格式。

use std::borrow::Cow;

/// 识别级别时每行最多检查的字节数
pub const HEAD: usize = 256;

pub const NONE: u8 = 0;
pub const TRACE: u8 = 1;
pub const DEBUG: u8 = 2;
pub const INFO: u8 = 3;
pub const WARN: u8 = 4;
pub const ERROR: u8 = 5;
pub const NAMES: [&str; 6] = ["none", "trace", "debug", "info", "warn", "error"];

const WORDS: &[(&[u8], u8)] = &[
    (b"FATAL", ERROR),
    (b"PANIC", ERROR),
    (b"CRITICAL", ERROR),
    (b"CRIT", ERROR),
    (b"SEVERE", ERROR),
    (b"EMERG", ERROR),
    (b"ALERT", ERROR),
    (b"ERROR", ERROR),
    (b"ERR", ERROR),
    (b"WARNING", WARN),
    (b"WARN", WARN),
    (b"NOTICE", INFO),
    (b"INFO", INFO),
    (b"DEBUG", DEBUG),
    (b"FINE", DEBUG),
    (b"TRACE", TRACE),
    (b"VERBOSE", TRACE),
    (b"FINER", TRACE),
    (b"FINEST", TRACE),
];

/// 小写 / 首字母大写的级别词只有紧跟在这些标记后面才算数，避免把正文里的 "no error" 当成错误
const MARKERS: &[&[u8]] = &[
    b"level=",
    b"lvl=",
    b"severity=",
    b"level\":\"",
    b"level\": \"",
    b"severity\":\"",
    b"severity\": \"",
    b"[",
    b"<",
];

/// 去掉 ANSI 颜色等控制序列
pub fn strip_ansi(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&0x1b) {
        return Cow::Borrowed(bytes);
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        match bytes.get(i + 1) {
            // CSI：ESC [ 参数 … 结束字节 0x40-0x7e
            Some(b'[') => {
                i += 2;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                i += 1;
            }
            // OSC：ESC ] … BEL 或 ESC \
            Some(b']') => {
                i += 2;
                while i < bytes.len()
                    && bytes[i] != 0x07
                    && !(bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\'))
                {
                    i += 1;
                }
                i += if bytes.get(i) == Some(&0x1b) { 2 } else { 1 };
            }
            _ => i += 2,
        }
    }
    Cow::Owned(out)
}

fn ends_with_ignore_case(text: &[u8], suffix: &[u8]) -> bool {
    text.len() >= suffix.len() && text[text.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

fn word_level(token: &[u8], exact: bool) -> Option<u8> {
    if !(3..=8).contains(&token.len()) {
        return None;
    }
    WORDS.iter().find_map(|(word, level)| {
        let hit = if exact {
            token == *word
        } else {
            token.eq_ignore_ascii_case(word)
        };
        hit.then_some(*level)
    })
}

/// pino 等 JSON 日志的数字级别
fn numeric_level(token: &[u8]) -> Option<u8> {
    match token {
        b"10" => Some(TRACE),
        b"20" => Some(DEBUG),
        b"30" => Some(INFO),
        b"40" => Some(WARN),
        b"50" | b"60" => Some(ERROR),
        _ => None,
    }
}

pub fn detect(line: &[u8]) -> u8 {
    let clean = strip_ansi(&line[..line.len().min(HEAD)]);
    let head = &clean[..];
    let mut i = 0;
    while i < head.len() {
        if !head[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < head.len() && head[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let token = &head[start..i];
        // 数字开头的片段（时间戳等）不可能是级别词，只需检查 JSON 数字级别
        if token[0].is_ascii_digit() {
            if token.len() == 2
                && (ends_with_ignore_case(&head[..start], b"\"level\":")
                    || ends_with_ignore_case(&head[..start], b"\"level\": "))
                && let Some(level) = numeric_level(token)
            {
                return level;
            }
            continue;
        }
        if let Some(level) = word_level(token, true) {
            return level;
        }
        if MARKERS
            .iter()
            .any(|m| ends_with_ignore_case(&head[..start], m))
            && let Some(level) = word_level(token, false)
        {
            return level;
        }
    }
    NONE
}

/// 堆栈等续行：沿用上一行的级别。
/// 日志记录通常以时间戳、[ 、{ 或 < 开头；不像记录开头又识别不到级别的行视为上一条记录的延续，
/// 例如 Java 异常的首行 `java.sql.SQLException: …` 和 Python 的 `Traceback …`
pub fn is_continuation(line: &[u8]) -> bool {
    match line.first() {
        None => false,
        Some(first) => !(first.is_ascii_digit() || matches!(first, b'[' | b'{' | b'<')),
    }
}

/// 级别：识别不到时对续行沿用上一行
pub fn classify(line: &[u8], previous: u8) -> u8 {
    match detect(line) {
        NONE if is_continuation(line) => previous,
        level => level,
    }
}

#[cfg(test)]
#[path = "level_test.rs"]
mod tests;
