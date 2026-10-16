//! 占位符提取：{{name}}（i18next）、{name} / {0} / {count, plural, ...}（ICU、ARB、MessageFormat）、
//! %s %d %1$s %@（printf）、%{name}（Ruby）。复数分支内的文本不算占位符。
//!
//! 具名与带位置的占位符只看是否出现；不带位置的 printf 占位符按顺序消费参数，出现次数也必须一致。

use std::collections::BTreeMap;

/// 占位符（规范化写法）→ 出现次数
pub type Placeholders = BTreeMap<String, usize>;

fn add(out: &mut Placeholders, token: String, counted: bool) {
    let count = out.entry(token).or_default();
    *count = if counted { *count + 1 } else { 1 };
}

/// 文本中出现的占位符
pub fn extract(text: &str) -> Placeholders {
    let mut out = Placeholders::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                // {{name}} / {{name, format}} / {{- name}}
                let start = i + 2;
                match find_seq(&chars, start, &['}', '}']) {
                    Some(end) => {
                        let inner: String = chars[start..end].iter().collect();
                        let name = inner
                            .trim_start_matches('-')
                            .split(',')
                            .next()
                            .unwrap_or_default()
                            .trim();
                        if !name.is_empty() {
                            add(&mut out, format!("{{{{{name}}}}}"), false);
                        }
                        i = end + 2;
                    }
                    None => i += 2,
                }
            }
            '{' => {
                // ICU：取逗号或右括号前的参数名，跳过整个（可能嵌套的）块
                let start = i + 1;
                let name_end = chars[start..]
                    .iter()
                    .position(|c| matches!(c, ',' | '}' | '{'))
                    .map(|p| start + p);
                match name_end {
                    Some(end) if chars[end] != '{' => {
                        let name: String = chars[start..end].iter().collect::<String>();
                        let name = name.trim();
                        if is_identifier(name) {
                            add(&mut out, format!("{{{name}}}"), false);
                        }
                        i = skip_block(&chars, i);
                    }
                    _ => i += 1,
                }
            }
            '%' => {
                let (len, token) = printf(&chars[i..]);
                if let Some(token) = token {
                    // %s 这类不带位置的占位符按顺序取参数，次数有意义
                    let counted = !token.contains('$') && !token.starts_with("%{");
                    add(&mut out, token, counted);
                }
                i += len.max(1);
            }
            _ => i += 1,
        }
    }
    out
}

fn is_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn find_seq(chars: &[char], from: usize, seq: &[char]) -> Option<usize> {
    (from..chars.len().saturating_sub(seq.len() - 1)).find(|&i| chars[i..].starts_with(seq))
}

/// 从 { 开始跳到配对的 } 之后
fn skip_block(chars: &[char], open: usize) -> usize {
    let mut depth = 0usize;
    for (offset, c) in chars[open..].iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset + 1;
                }
            }
            _ => {}
        }
    }
    chars.len()
}

/// printf 风格：%%、%s、%d、%1$s、%.2f、%@、%{name}
fn printf(chars: &[char]) -> (usize, Option<String>) {
    match chars.get(1) {
        Some('%') => return (2, None),
        Some('{') => {
            if let Some(end) = chars.iter().position(|c| *c == '}') {
                let name: String = chars[2..end].iter().collect();
                if is_identifier(&name) {
                    return (end + 1, Some(format!("%{{{name}}}")));
                }
            }
            return (1, None);
        }
        _ => {}
    }
    let mut i = 1;
    let mut position = String::new();
    while chars.get(i).is_some_and(char::is_ascii_digit) {
        position.push(chars[i]);
        i += 1;
    }
    if chars.get(i) == Some(&'$') {
        i += 1;
    } else {
        // 不是位置参数：数字属于宽度
        position.clear();
        i = 1;
    }
    while chars
        .get(i)
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | '#' | 'l' | 'h'))
    {
        i += 1;
    }
    match chars.get(i) {
        Some(conv @ ('s' | 'd' | 'i' | 'f' | 'u' | 'x' | 'X' | 'o' | 'e' | 'g' | 'c' | '@')) => {
            // 宽度与精度不影响参数是否一致；只比较位置与类型
            let token = if position.is_empty() {
                format!("%{conv}")
            } else {
                format!("%{position}${conv}")
            };
            (i + 1, Some(token))
        }
        _ => (1, None),
    }
}

#[cfg(test)]
#[path = "placeholders_test.rs"]
mod tests;
