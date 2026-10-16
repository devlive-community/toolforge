//! 轻量 SQL 扫描：识别字符串 / 引号标识符 / 注释，用于压缩与语句计数。

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    /// 字符串、引号标识符、dollar-quoted 字符串等必须原样保留的片段
    Literal(&'a str),
    Comment,
    Whitespace,
    Other(char),
}

pub fn tokenize(sql: &str) -> Vec<Token<'_>> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < sql.len() {
        let rest = &sql[i..];
        let ch = rest.chars().next().expect("non-empty");
        let len = match ch {
            '\'' | '"' | '`' => quoted(rest, ch, ch),
            '[' => quoted(rest, '[', ']'),
            '-' if rest.starts_with("--") => {
                let end = rest.find('\n').unwrap_or(rest.len());
                tokens.push(Token::Comment);
                i += end;
                continue;
            }
            '/' if rest.starts_with("/*") => {
                let end = rest[2..].find("*/").map(|p| p + 4).unwrap_or(rest.len());
                tokens.push(Token::Comment);
                i += end;
                continue;
            }
            '$' => dollar_quoted(rest).unwrap_or(0),
            c if c.is_whitespace() => {
                let end = rest
                    .find(|c: char| !c.is_whitespace())
                    .unwrap_or(rest.len());
                tokens.push(Token::Whitespace);
                i += end;
                continue;
            }
            _ => 0,
        };
        if len > 0 {
            tokens.push(Token::Literal(&sql[i..i + len]));
            i += len;
        } else {
            tokens.push(Token::Other(ch));
            i += ch.len_utf8();
        }
        debug_assert!(i <= bytes.len());
    }
    tokens
}

/// 引号片段长度；成对的结束引号视为转义（'it''s'）
fn quoted(rest: &str, open: char, close: char) -> usize {
    let mut chars = rest.char_indices().skip(1).peekable();
    while let Some((index, c)) = chars.next() {
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
pub fn minify(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut pending_space = false;
    for token in tokenize(sql) {
        match token {
            Token::Whitespace | Token::Comment => pending_space = true,
            Token::Literal(text) => {
                push_space(&mut out, pending_space, None);
                out.push_str(text);
                pending_space = false;
            }
            Token::Other(c) => {
                push_space(&mut out, pending_space, Some(c));
                out.push(c);
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
pub fn count_statements(sql: &str) -> usize {
    let mut count = 0;
    let mut has_content = false;
    for token in tokenize(sql) {
        match token {
            Token::Other(';') => {
                if has_content {
                    count += 1;
                }
                has_content = false;
            }
            Token::Whitespace | Token::Comment => {}
            _ => has_content = true,
        }
    }
    count + usize::from(has_content)
}

#[cfg(test)]
#[path = "scan_test.rs"]
mod tests;
