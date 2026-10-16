//! 格式化前把引擎无法识别的写法替换为反引号占位符（引擎把它当作引号标识符原样保留），
//! 格式化后还原；方言子句短语还原时独立成行。

use crate::dialect::{DialectKind, Placement};
use crate::format::KeywordCase;
use crate::scan::{self, Kind, Token};

enum Slot {
    /// 原样还原
    Verbatim(String),
    /// 方言短语：按大小写选项输出关键字
    Phrase {
        words: &'static [&'static str],
        original: String,
        placement: Placement,
    },
    /// MySQL ON DUPLICATE KEY UPDATE 中的 VALUES(col)：VALUES 按大小写选项输出，其余原样
    Call { keyword: String, rest: String },
}

pub struct Protected {
    pub text: String,
    slots: Vec<Slot>,
    prefix: String,
}

/// 选一个原文中不存在的占位前缀
fn unique_prefix(input: &str) -> String {
    (0..)
        .map(|n| format!("__tf{n}ph"))
        .find(|p| !input.contains(p.as_str()))
        .expect("unbounded")
}

fn case_word(word: &str, case: KeywordCase) -> String {
    match case {
        KeywordCase::Upper => word.to_uppercase(),
        KeywordCase::Lower => word.to_lowercase(),
        KeywordCase::Preserve => word.to_owned(),
    }
}

/// tokens[i..] 开头是否为该短语（单词之间只允许空白），返回短语占用的记号数
fn match_phrase(tokens: &[Token], i: usize, words: &[&str]) -> Option<usize> {
    let mut j = i;
    for (n, word) in words.iter().enumerate() {
        if n > 0 {
            if tokens.get(j)?.kind != Kind::Whitespace {
                return None;
            }
            j += 1;
        }
        let token = tokens.get(j)?;
        if token.kind != Kind::Word || !token.text.eq_ignore_ascii_case(word) {
            return None;
        }
        j += 1;
    }
    Some(j - i)
}

/// 从 tokens[i]（左括号或左尖括号所在位置）开始找到配对的右括号，返回占用的记号数
fn balanced(tokens: &[Token], i: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, token) in tokens[i..].iter().enumerate() {
        if token.kind != Kind::Other {
            continue;
        }
        if token.text == open {
            depth += 1;
        } else if token.text == close {
            depth -= 1;
            if depth == 0 {
                return Some(offset + 1);
            }
        }
    }
    None
}

fn next_non_space(tokens: &[Token], mut i: usize) -> usize {
    while tokens.get(i).is_some_and(|t| t.kind == Kind::Whitespace) {
        i += 1;
    }
    i
}

fn join(tokens: &[Token]) -> String {
    tokens.iter().map(|t| t.text).collect()
}

pub fn protect(input: &str, dialect: DialectKind) -> Protected {
    let tokens = scan::tokenize(input, dialect);
    let prefix = unique_prefix(input);
    let mut text = String::with_capacity(input.len());
    let mut slots = Vec::new();
    let mut push = |text: &mut String, slot: Slot| {
        text.push_str(&format!("`{prefix}{}`", slots.len()));
        slots.push(slot);
    };
    let mut after_upsert = false;
    let mut i = 0;
    'outer: while i < tokens.len() {
        let token = tokens[i];
        match token.kind {
            Kind::Literal { special: true } => {
                push(&mut text, Slot::Verbatim(token.text.to_owned()));
                i += 1;
                continue;
            }
            Kind::Other if token.text == ";" => after_upsert = false,
            Kind::Word => {
                for phrase in dialect.phrases() {
                    if let Some(len) = match_phrase(&tokens, i, phrase.words) {
                        if phrase.words[0] == "ON" && phrase.words[1] == "DUPLICATE" {
                            after_upsert = true;
                        }
                        push(
                            &mut text,
                            Slot::Phrase {
                                words: phrase.words,
                                original: join(&tokens[i..i + len]),
                                placement: phrase.placement,
                            },
                        );
                        i += len;
                        continue 'outer;
                    }
                }
                // ARRAY<INT64> / STRUCT<a INT64>：尖括号内整体保留
                if dialect.angle_types()
                    && (token.text.eq_ignore_ascii_case("ARRAY")
                        || token.text.eq_ignore_ascii_case("STRUCT"))
                {
                    let open = next_non_space(&tokens, i + 1);
                    if tokens.get(open).is_some_and(|t| t.text == "<")
                        && let Some(len) = balanced(&tokens, open, "<", ">")
                    {
                        let end = open + len;
                        push(
                            &mut text,
                            Slot::Call {
                                keyword: token.text.to_owned(),
                                rest: join(&tokens[i + 1..end]),
                            },
                        );
                        i = end;
                        continue;
                    }
                }
                // ON DUPLICATE KEY UPDATE 之后的 VALUES(col) 是函数，不是 VALUES 子句
                if after_upsert && token.text.eq_ignore_ascii_case("VALUES") {
                    let open = next_non_space(&tokens, i + 1);
                    if tokens.get(open).is_some_and(|t| t.text == "(")
                        && let Some(len) = balanced(&tokens, open, "(", ")")
                    {
                        let end = open + len;
                        push(
                            &mut text,
                            Slot::Call {
                                keyword: token.text.to_owned(),
                                rest: join(&tokens[i + 1..end]),
                            },
                        );
                        i = end;
                        continue;
                    }
                }
            }
            _ => {}
        }
        text.push_str(token.text);
        i += 1;
    }
    Protected {
        text,
        slots,
        prefix,
    }
}

/// 引擎关键字表之外的方言关键字按大小写选项输出（占位符、字符串与注释不受影响）
pub fn case_keywords(sql: &str, dialect: DialectKind, case: KeywordCase) -> String {
    let keywords = dialect.keywords();
    if case == KeywordCase::Preserve || keywords.is_empty() {
        return sql.to_owned();
    }
    scan::tokenize(sql, dialect)
        .into_iter()
        .map(|token| {
            if token.kind == Kind::Word
                && keywords.iter().any(|k| k.eq_ignore_ascii_case(token.text))
            {
                case_word(token.text, case)
            } else {
                token.text.to_owned()
            }
        })
        .collect()
}

/// 行首空白
fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// 去掉一级缩进
fn outdent<'a>(indent: &'a str, unit: &str) -> &'a str {
    indent.strip_suffix(unit).unwrap_or("")
}

impl Protected {
    pub fn restore(
        &self,
        formatted: &str,
        dialect: DialectKind,
        case: KeywordCase,
        unit: &str,
    ) -> String {
        let mut out = formatted.to_owned();
        for (n, slot) in self.slots.iter().enumerate() {
            let placeholder = format!("`{}{n}`", self.prefix);
            let Some(at) = out.find(&placeholder) else {
                continue;
            };
            let before = &out[..at];
            let after = &out[at + placeholder.len()..];
            out = match slot {
                Slot::Verbatim(text) => format!("{before}{text}{after}"),
                Slot::Call { keyword, rest } => format!(
                    "{before}{}{}{after}",
                    case_word(keyword, case),
                    case_keywords(rest, dialect, case)
                ),
                Slot::Phrase {
                    words,
                    original,
                    placement,
                } => {
                    let phrase = match case {
                        KeywordCase::Preserve => {
                            original.split_whitespace().collect::<Vec<_>>().join(" ")
                        }
                        _ => words
                            .iter()
                            .map(|w| case_word(w, case))
                            .collect::<Vec<_>>()
                            .join(" "),
                    };
                    match placement {
                        Placement::Inline => format!("{before}{phrase}{after}"),
                        Placement::Clause => clause_lines(before, &phrase, after, unit),
                    }
                }
            };
        }
        out
    }
}

/// 子句短语独立成行，后面的内容另起一行并缩进一级
fn clause_lines(before: &str, phrase: &str, after: &str, unit: &str) -> String {
    let line_start = before.rfind('\n').map_or(0, |p| p + 1);
    let line = &before[line_start..];
    let base = if line.trim().is_empty() {
        outdent(line, unit)
    } else {
        outdent(indent_of(line), unit)
    };
    let head = before[..line_start].to_owned() + line.trim_end();
    let mut out = head;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(base);
    out.push_str(phrase);
    let rest = after.trim_start_matches([' ', '\t']);
    if rest.is_empty() || rest.starts_with(['\n', ';', ')']) {
        out.push_str(rest);
    } else if rest.starts_with('(') {
        // ON CONFLICT (id)：紧跟的括号留在同一行
        out.push(' ');
        out.push_str(rest);
    } else {
        out.push('\n');
        out.push_str(base);
        out.push_str(unit);
        out.push_str(rest);
    }
    out
}

#[cfg(test)]
#[path = "protect_test.rs"]
mod tests;
