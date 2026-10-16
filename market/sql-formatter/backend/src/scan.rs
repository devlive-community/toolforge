//! 轻量 SQL 扫描：按方言识别字符串 / 引号标识符 / 注释 / 单词，用于压缩、语句计数、
//! 占位保护与关键字大小写处理。每个记号都是原文的切片。

use crate::dialect::DialectKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 字符串、引号标识符、dollar-quoted 字符串等必须原样保留的片段；
    /// special 表示格式化引擎无法正确识别、需要占位保护
    Literal {
        special: bool,
    },
    Comment,
    Whitespace,
    /// 字母、数字、下划线组成的单词
    Word,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: Kind,
    pub text: &'a str,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

pub fn tokenize(sql: &str, dialect: DialectKind) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < sql.len() {
        let rest = &sql[i..];
        let ch = rest.chars().next().expect("non-empty");
        let (kind, len) = scan_one(rest, ch, dialect);
        let len = len.max(ch.len_utf8());
        tokens.push(Token {
            kind,
            text: &sql[i..i + len],
        });
        i += len;
    }
    tokens
}

fn literal(len: usize, special: bool) -> (Kind, usize) {
    (Kind::Literal { special }, len)
}

fn scan_one(rest: &str, ch: char, dialect: DialectKind) -> (Kind, usize) {
    let escapes = dialect.backslash_escapes();
    match ch {
        '\'' | '"' if dialect.bigquery_strings() && triple(rest).is_some() => {
            literal(triple(rest).unwrap_or(rest.len()), true)
        }
        '\'' | '"' => literal(quoted(rest, ch, ch, escapes), false),
        '`' => literal(quoted(rest, '`', '`', false), false),
        '[' if dialect.bracket_identifiers() => {
            literal(quoted(rest, '[', ']', false), dialect.protect_brackets())
        }
        '#' if dialect.hash_comments() => (Kind::Comment, line_end(rest)),
        '-' if rest.starts_with("--") => (Kind::Comment, line_end(rest)),
        '/' if rest.starts_with("/*") => (
            Kind::Comment,
            rest[2..].find("*/").map_or(rest.len(), |p| p + 4),
        ),
        '$' if dialect.dollar_quotes() && dollar_quoted(rest).is_some() => {
            literal(dollar_quoted(rest).unwrap_or(0), false)
        }
        c if c.is_whitespace() => (
            Kind::Whitespace,
            rest.find(|c: char| !c.is_whitespace())
                .unwrap_or(rest.len()),
        ),
        c if is_word_char(c) => {
            if let Some(len) = prefixed_string(rest, dialect) {
                return literal(len, true);
            }
            (
                Kind::Word,
                rest.find(|c: char| !is_word_char(c)).unwrap_or(rest.len()),
            )
        }
        _ => (Kind::Other, ch.len_utf8()),
    }
}

fn line_end(rest: &str) -> usize {
    rest.find('\n').unwrap_or(rest.len())
}

/// 引号片段长度；成对的结束引号视为转义（'it''s'），escapes 时 \ 转义下一个字符
fn quoted(rest: &str, open: char, close: char, escapes: bool) -> usize {
    let mut chars = rest.char_indices().skip(1).peekable();
    while let Some((index, c)) = chars.next() {
        if escapes && c == '\\' {
            chars.next();
            continue;
        }
        if c == close {
            if open == close && chars.peek().map(|(_, n)| *n) == Some(close) {
                chars.next();
                continue;
            }
            return index + close.len_utf8();
        }
    }
    rest.len()
}

/// BigQuery '''…''' 与 """…"""
fn triple(rest: &str) -> Option<usize> {
    let quote = rest.get(..3).filter(|q| *q == "'''" || *q == "\"\"\"")?;
    let body = &rest[3..];
    let mut i = 0;
    while i < body.len() {
        if body[i..].starts_with('\\') {
            i += 1 + body[i + 1..].chars().next().map_or(0, char::len_utf8);
            continue;
        }
        if body[i..].starts_with(quote) {
            return Some(3 + i + 3);
        }
        i += body[i..].chars().next().map_or(1, char::len_utf8);
    }
    Some(rest.len())
}

/// 带前缀的字符串：BigQuery r'' / b'' / rb''，Oracle q'[...]' / nq'[...]'
fn prefixed_string(rest: &str, dialect: DialectKind) -> Option<usize> {
    let prefix_len = rest
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_alphabetic())
        .count();
    if prefix_len == 0 || prefix_len > 2 {
        return None;
    }
    let prefix = rest[..prefix_len].to_ascii_lowercase();
    let after = &rest[prefix_len..];
    let quote = after.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    if dialect.bigquery_strings() && matches!(prefix.as_str(), "r" | "b" | "rb" | "br") {
        let raw = prefix.contains('r');
        let body = match triple(after) {
            Some(len) => len,
            None => quoted(after, quote, quote, !raw),
        };
        return Some(prefix_len + body);
    }
    if dialect.q_quotes() && matches!(prefix.as_str(), "q" | "nq") && quote == '\'' {
        let open = after[1..].chars().next()?;
        let close = match open {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            '<' => '>',
            c if c.is_whitespace() => return None,
            c => c,
        };
        let body_start = 1 + open.len_utf8();
        let end = after[body_start..]
            .find(&format!("{close}'"))
            .map_or(after.len(), |p| body_start + p + close.len_utf8() + 1);
        return Some(prefix_len + end);
    }
    None
}

/// PostgreSQL 的 $tag$ ... $tag$ 字符串
fn dollar_quoted(rest: &str) -> Option<usize> {
    let tag_end = rest[1..].find('$')? + 2;
    let tag = &rest[..tag_end];
    if !tag[1..tag.len() - 1]
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return None;
    }
    let body = rest[tag_end..].find(tag)?;
    Some(tag_end + body + tag.len())
}

const TIGHT: &[char] = &['(', ')', ',', ';', '.'];

/// 删除注释并合并空白；括号、逗号、分号、点两侧不留空格
pub fn minify(sql: &str, dialect: DialectKind) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut pending_space = false;
    for token in tokenize(sql, dialect) {
        match token.kind {
            Kind::Whitespace | Kind::Comment => pending_space = true,
            _ => {
                let first = token.text.chars().next();
                let next = if token.kind == Kind::Other {
                    first
                } else {
                    None
                };
                push_space(&mut out, pending_space, next);
                out.push_str(token.text);
                pending_space = false;
            }
        }
    }
    out.trim().to_owned()
}

fn push_space(out: &mut String, pending: bool, next: Option<char>) {
    let prev_tight = out
        .chars()
        .last()
        .is_none_or(|c| TIGHT.contains(&c) && c != ')');
    let next_tight = next.is_some_and(|c| TIGHT.contains(&c) && c != '(');
    if pending && !prev_tight && !next_tight {
        out.push(' ');
    }
}

/// 统计语句数：按字符串与注释之外的分号切分，忽略空语句
pub fn count_statements(sql: &str, dialect: DialectKind) -> usize {
    let mut count = 0;
    let mut has_content = false;
    for token in tokenize(sql, dialect) {
        match token.kind {
            Kind::Other if token.text == ";" => {
                if has_content {
                    count += 1;
                }
                has_content = false;
            }
            Kind::Whitespace | Kind::Comment => {}
            _ => has_content = true,
        }
    }
    count + usize::from(has_content)
}

#[cfg(test)]
#[path = "scan_test.rs"]
mod tests;
