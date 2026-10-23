//! 把命令行拆成参数：支持 bash（单双引号、`$'…'`、反斜杠续行）与
//! Windows cmd（浏览器「复制为 cURL (cmd)」产生的 `^` 转义）两种写法。

use tf_plugin_api::{PluginError, PluginResult};

fn unterminated() -> PluginError {
    PluginError::new("curl.unterminated_quote")
}

/// cmd 写法：出现 `^"` 或以 `^` 结尾的续行
fn is_cmd(input: &str) -> bool {
    input.contains("^\"") || input.lines().any(|line| line.trim_end().ends_with(" ^"))
}

pub fn tokenize(input: &str) -> PluginResult<Vec<String>> {
    if is_cmd(input) {
        tokenize_cmd(input)
    } else {
        tokenize_bash(input)
    }
}

fn hex_value(chars: &mut std::iter::Peekable<std::str::Chars>, max: usize) -> Option<u32> {
    let mut digits = String::new();
    while digits.len() < max {
        match chars.peek() {
            Some(c) if c.is_ascii_hexdigit() => digits.push(chars.next().unwrap()),
            _ => break,
        }
    }
    u32::from_str_radix(&digits, 16).ok()
}

/// `$'…'` 中的 ANSI-C 转义
fn ansi_c(chars: &mut std::iter::Peekable<std::str::Chars>, out: &mut String) -> PluginResult<()> {
    loop {
        match chars.next().ok_or_else(unterminated)? {
            '\'' => return Ok(()),
            '\\' => match chars.next().ok_or_else(unterminated)? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'a' => out.push('\x07'),
                'b' => out.push('\x08'),
                'e' | 'E' => out.push('\x1b'),
                'f' => out.push('\x0c'),
                'v' => out.push('\x0b'),
                'x' => {
                    if let Some(c) = hex_value(chars, 2).and_then(char::from_u32) {
                        out.push(c);
                    }
                }
                'u' | 'U' => {
                    if let Some(c) = hex_value(chars, 8).and_then(char::from_u32) {
                        out.push(c);
                    }
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
}

fn tokenize_bash(input: &str) -> PluginResult<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                // 反斜杠续行
                Some('\n') => {}
                Some('\r') if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                Some(next) => {
                    current.push(next);
                    started = true;
                }
                None => {}
            },
            '\'' => {
                started = true;
                loop {
                    match chars.next().ok_or_else(unterminated)? {
                        '\'' => break,
                        ch => current.push(ch),
                    }
                }
            }
            '$' if chars.peek() == Some(&'\'') => {
                chars.next();
                started = true;
                ansi_c(&mut chars, &mut current)?;
            }
            '"' => {
                started = true;
                loop {
                    match chars.next().ok_or_else(unterminated)? {
                        '"' => break,
                        '\\' => match chars.next().ok_or_else(unterminated)? {
                            '\n' => {}
                            ch @ ('"' | '\\' | '$' | '`') => current.push(ch),
                            ch => {
                                current.push('\\');
                                current.push(ch);
                            }
                        },
                        ch => current.push(ch),
                    }
                }
            }
            '#' if !started => {
                // 注释到行尾
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        break;
                    }
                }
            }
            // 管道、命令分隔与重定向之后不再属于 curl 的参数
            '|' | ';' | '&' | '>' | '<' => break,
            c if c.is_whitespace() => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }
    Ok(tokens)
}

fn tokenize_cmd(input: &str) -> PluginResult<Vec<String>> {
    // 先去掉 cmd 的 ^ 转义（^ 加换行为续行），再按 Windows 命令行规则拆分
    let mut plain = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '^' {
            match chars.next() {
                Some('\r') => {
                    chars.next();
                }
                Some('\n') | None => {}
                Some(next) => plain.push(next),
            }
        } else {
            plain.push(c);
        }
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = plain.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                // 反斜杠只在引号前有特殊含义
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                if chars.peek() == Some(&'"') {
                    current.extend(std::iter::repeat_n('\\', count / 2));
                    if count % 2 == 1 {
                        chars.next();
                        current.push('"');
                    }
                } else {
                    current.extend(std::iter::repeat_n('\\', count));
                }
                started = true;
            }
            '"' => {
                started = true;
                if quoted && chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    quoted = !quoted;
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(unterminated());
    }
    if started {
        tokens.push(current);
    }
    Ok(tokens)
}

#[cfg(test)]
#[path = "tokenize_test.rs"]
mod tests;
