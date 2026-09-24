use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
pub enum Style {
    /// \uXXXX，超出 BMP 使用代理对（JavaScript / JSON）
    #[default]
    #[serde(rename = "u")]
    U,
    /// \u{XXXX}（ES6 / Rust）
    #[serde(rename = "braces")]
    Braces,
    /// U+XXXX（码位表示法，空格分隔）
    #[serde(rename = "codepoint")]
    Codepoint,
}

pub fn escape(input: &str, style: Style, ascii_too: bool) -> String {
    // 码位表示法与十六进制字母存在歧义（U+1F600a），因此总是转义全部字符并以空格分隔
    if style == Style::Codepoint {
        return input
            .chars()
            .map(|ch| format!("U+{:04X}", ch as u32))
            .collect::<Vec<_>>()
            .join(" ");
    }
    let mut out = String::with_capacity(input.len() * 2);
    for ch in input.chars() {
        if ch.is_ascii() && !ascii_too {
            out.push(ch);
            continue;
        }
        match style {
            Style::U => {
                let mut units = [0u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            Style::Braces => out.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            Style::Codepoint => unreachable!("handled above"),
        }
    }
    out
}

fn invalid(at: usize) -> PluginError {
    PluginError::new("encode.invalid_escape").with("position", at)
}

fn hex(s: &str, at: usize) -> PluginResult<u32> {
    u32::from_str_radix(s, 16).map_err(|_| invalid(at))
}

fn push_code(out: &mut String, code: u32, at: usize) -> PluginResult<()> {
    out.push(char::from_u32(code).ok_or_else(|| invalid(at))?);
    Ok(())
}

/// 解析 \uXXXX（含代理对）、\u{…}、\UXXXXXXXX、\xHH 与 U+XXXX；其他字符原样保留
pub fn unescape(input: &str) -> PluginResult<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let take = |from: usize, n: usize| -> Option<String> {
        (from + n <= chars.len()).then(|| chars[from..from + n].iter().collect())
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'u' if chars.get(i + 2) == Some(&'{') => {
                    let end = chars[i + 3..]
                        .iter()
                        .position(|&c| c == '}')
                        .ok_or_else(|| invalid(i))?;
                    let digits: String = chars[i + 3..i + 3 + end].iter().collect();
                    push_code(&mut out, hex(&digits, i)?, i)?;
                    i += 4 + end;
                    continue;
                }
                'u' => {
                    let high = hex(&take(i + 2, 4).ok_or_else(|| invalid(i))?, i)?;
                    if (0xD800..0xDC00).contains(&high) {
                        // 高位代理必须紧跟 \u 低位代理
                        let low = match (take(i + 6, 2).as_deref(), take(i + 8, 4)) {
                            (Some("\\u"), Some(digits)) => hex(&digits, i + 6)?,
                            _ => return Err(invalid(i)),
                        };
                        if !(0xDC00..0xE000).contains(&low) {
                            return Err(invalid(i + 6));
                        }
                        push_code(
                            &mut out,
                            0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00),
                            i,
                        )?;
                        i += 12;
                    } else {
                        push_code(&mut out, high, i)?;
                        i += 6;
                    }
                    continue;
                }
                'U' => {
                    let code = hex(&take(i + 2, 8).ok_or_else(|| invalid(i))?, i)?;
                    push_code(&mut out, code, i)?;
                    i += 10;
                    continue;
                }
                'x' => {
                    let code = hex(&take(i + 2, 2).ok_or_else(|| invalid(i))?, i)?;
                    push_code(&mut out, code, i)?;
                    i += 4;
                    continue;
                }
                _ => {}
            }
        }
        if c == 'U' && chars.get(i + 1) == Some(&'+') {
            let digits: String = chars[i + 2..]
                .iter()
                .take_while(|c| c.is_ascii_hexdigit())
                .take(6)
                .collect();
            if digits.len() >= 4 {
                push_code(&mut out, hex(&digits, i)?, i)?;
                i += 2 + digits.len();
                // U+XXXX 之间的单个分隔空格不属于内容
                if chars.get(i) == Some(&' ')
                    && chars.get(i + 1) == Some(&'U')
                    && chars.get(i + 2) == Some(&'+')
                {
                    i += 1;
                }
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
#[path = "unicode_test.rs"]
mod tests;
