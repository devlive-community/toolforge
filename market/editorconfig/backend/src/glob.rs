//! EditorConfig 规范的 glob：`*` 不跨目录、`**` 跨目录、`?` 单个字符、`[abc]` / `[!abc]` 字符集、
//! `{s1,s2}` 备选（可嵌套）、`{num1..num2}` 整数范围、`\` 转义。
//!
//! 不含 `/` 的 glob 可以匹配该 .editorconfig 所在目录及以下任意层级的文件名；
//! 含 `/` 的 glob 相对于 .editorconfig 所在目录（开头的 `/` 可省略）。

use regex::Regex;

pub struct Glob {
    re: Regex,
    /// 每个数字范围对应的捕获组序号与上下界
    ranges: Vec<(usize, i64, i64)>,
}

struct Builder {
    out: String,
    ranges: Vec<(usize, i64, i64)>,
    groups: usize,
}

/// 找到与 chars[open] 处 `{` 配对的 `}`（考虑转义与嵌套）
fn closing_brace(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = open;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 按顶层逗号拆分花括号内容
fn split_alternatives(chars: &[char]) -> Vec<&[char]> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&chars[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&chars[start..]);
    parts
}

fn numeric_range(chars: &[char]) -> Option<(i64, i64)> {
    let text: String = chars.iter().collect();
    let (a, b) = text.split_once("..")?;
    let parse = |s: &str| -> Option<i64> {
        let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
        (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let (a, b) = (parse(a)?, parse(b)?);
    Some((a.min(b), a.max(b)))
}

impl Builder {
    fn translate(&mut self, chars: &[char]) {
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            match c {
                '\\' if i + 1 < chars.len() => {
                    self.out.push_str(&regex::escape(&chars[i + 1].to_string()));
                    i += 2;
                    continue;
                }
                '*' if chars.get(i + 1) == Some(&'*') => {
                    // a/**/b 也匹配 a/b
                    if chars.get(i + 2) == Some(&'/') {
                        self.out.push_str("(?:.*/)?");
                        i += 3;
                    } else {
                        self.out.push_str(".*");
                        i += 2;
                    }
                    continue;
                }
                '*' => self.out.push_str("[^/]*"),
                '?' => self.out.push_str("[^/]"),
                '[' => {
                    if let Some(len) = self.class(&chars[i..]) {
                        i += len;
                        continue;
                    }
                    self.out.push_str("\\[");
                }
                '{' => {
                    if let Some(close) = closing_brace(chars, i) {
                        let inner = &chars[i + 1..close];
                        if let Some((low, high)) = numeric_range(inner) {
                            self.groups += 1;
                            self.ranges.push((self.groups, low, high));
                            self.out.push_str("([+-]?\\d+)");
                        } else {
                            let parts = split_alternatives(inner);
                            if parts.len() < 2 {
                                // {single} 不是备选，按字面匹配
                                self.out.push_str("\\{");
                                self.translate(inner);
                                self.out.push_str("\\}");
                            } else {
                                self.out.push_str("(?:");
                                for (n, part) in parts.iter().enumerate() {
                                    if n > 0 {
                                        self.out.push('|');
                                    }
                                    self.translate(part);
                                }
                                self.out.push(')');
                            }
                        }
                        i = close + 1;
                        continue;
                    }
                    self.out.push_str("\\{");
                }
                other => self.out.push_str(&regex::escape(&other.to_string())),
            }
            i += 1;
        }
    }

    /// 字符集；不闭合或包含 / 时返回 None（按字面处理）
    fn class(&mut self, chars: &[char]) -> Option<usize> {
        let mut i = 1;
        let negated = matches!(chars.get(1), Some('!' | '^'));
        if negated {
            i += 1;
        }
        let start = i;
        let mut body = String::new();
        while i < chars.len() {
            match chars[i] {
                ']' if i > start => {
                    self.out.push('[');
                    if negated {
                        self.out.push('^');
                    }
                    self.out.push_str(&body);
                    self.out.push(']');
                    return Some(i + 1);
                }
                '/' => return None,
                '\\' if i + 1 < chars.len() => {
                    body.push('\\');
                    body.push(chars[i + 1]);
                    i += 1;
                }
                c @ ('[' | '^' | '&' | '~') => {
                    body.push('\\');
                    body.push(c);
                }
                c => body.push(c),
            }
            i += 1;
        }
        None
    }
}

impl Glob {
    pub fn new(pattern: &str) -> Option<Self> {
        let chars: Vec<char> = pattern.chars().collect();
        // 字符集里的 / 不算路径分隔符；这里与常见实现一致，只看是否出现 /
        let has_slash = pattern.contains('/');
        let body: &[char] = if chars.first() == Some(&'/') {
            &chars[1..]
        } else {
            &chars
        };
        let mut builder = Builder {
            out: String::from("^"),
            ranges: Vec::new(),
            groups: 0,
        };
        if !has_slash {
            builder.out.push_str("(?:.*/)?");
        }
        builder.translate(body);
        builder.out.push('$');
        Some(Self {
            re: Regex::new(&builder.out).ok()?,
            ranges: builder.ranges,
        })
    }

    /// path 为相对 .editorconfig 所在目录、以 / 分隔的路径
    pub fn matches(&self, path: &str) -> bool {
        let Some(caps) = self.re.captures(path) else {
            return false;
        };
        self.ranges.iter().all(|(group, low, high)| {
            caps.get(*group)
                .and_then(|m| m.as_str().parse::<i64>().ok())
                .is_some_and(|n| (*low..=*high).contains(&n))
        })
    }
}

#[cfg(test)]
#[path = "glob_test.rs"]
mod tests;
