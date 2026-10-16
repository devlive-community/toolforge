//! RFC 8785 JSON 规范化（JCS）。
//!
//! 自带严格解析器而不复用 serde_json：需要从原文发现重复键与孤立代理项，
//! 并按 binary64 语义读取数字（serde_json 在本工作区开启了任意精度）。

use tf_plugin_api::{PluginError, PluginResult};

const MAX_DEPTH: usize = 512;

#[derive(Debug, PartialEq)]
enum Node {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    /// 数字的有效数字与规范输出不一致（精度变化）的次数
    lossy: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            lossy: 0,
        }
    }

    fn error_at(&self, code: &'static str, pos: usize) -> PluginError {
        let before = &self.src[..self.src.floor_char_boundary(pos.min(self.src.len()))];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        PluginError::new(code)
            .with("line", line)
            .with("column", column)
    }

    fn error(&self, code: &'static str) -> PluginError {
        self.error_at(code, self.pos)
    }

    fn syntax(&self, detail: &str) -> PluginError {
        let code = if self.pos >= self.bytes.len() {
            "json.eof"
        } else {
            "json.syntax"
        };
        self.error(code).with("detail", detail.to_owned())
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.pos) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, literal: &str) -> PluginResult<()> {
        if self.src[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(self.syntax(&format!("expected `{literal}`")))
        }
    }

    fn value(&mut self, depth: usize) -> PluginResult<Node> {
        if depth > MAX_DEPTH {
            return Err(self.error("json.too_deep").with("max", MAX_DEPTH));
        }
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Node::String),
            Some(b't') => self.expect("true").map(|_| Node::Bool(true)),
            Some(b'f') => self.expect("false").map(|_| Node::Bool(false)),
            Some(b'n') => self.expect("null").map(|_| Node::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.syntax("expected a value")),
        }
    }

    fn object(&mut self, depth: usize) -> PluginResult<Node> {
        self.pos += 1;
        let mut members: Vec<(String, Node)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Node::Object(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.syntax("expected a string key"));
            }
            let key_pos = self.pos;
            let key = self.string()?;
            if members.iter().any(|(k, _)| *k == key) {
                return Err(self
                    .error_at("json.duplicate_key", key_pos)
                    .with("key", key));
            }
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.syntax("expected `:`"));
            }
            self.pos += 1;
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Node::Object(members));
                }
                _ => return Err(self.syntax("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> PluginResult<Node> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Node::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Node::Array(items));
                }
                _ => return Err(self.syntax("expected `,` or `]`")),
            }
        }
    }

    fn hex4(&mut self) -> PluginResult<u16> {
        let digits = self
            .src
            .get(self.pos..self.pos + 4)
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| self.syntax("invalid \\u escape"))?;
        self.pos += 4;
        Ok(u16::from_str_radix(digits, 16).expect("hex digits"))
    }

    fn string(&mut self) -> PluginResult<String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(&b) = self.bytes.get(self.pos) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(&self.src[start..self.pos]);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    let escape_pos = self.pos;
                    self.pos += 1;
                    let c = self
                        .peek()
                        .ok_or_else(|| self.syntax("unterminated string"))?;
                    self.pos += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            let ch = match unit {
                                0xD800..=0xDBFF => {
                                    if !self.src[self.pos..].starts_with("\\u") {
                                        return Err(
                                            self.error_at("json.lone_surrogate", escape_pos)
                                        );
                                    }
                                    self.pos += 2;
                                    let low = self.hex4()?;
                                    if !(0xDC00..=0xDFFF).contains(&low) {
                                        return Err(
                                            self.error_at("json.lone_surrogate", escape_pos)
                                        );
                                    }
                                    let code = 0x10000
                                        + ((u32::from(unit) - 0xD800) << 10)
                                        + (u32::from(low) - 0xDC00);
                                    char::from_u32(code).expect("valid surrogate pair")
                                }
                                0xDC00..=0xDFFF => {
                                    return Err(self.error_at("json.lone_surrogate", escape_pos));
                                }
                                _ => char::from_u32(u32::from(unit)).expect("BMP scalar"),
                            };
                            out.push(ch);
                        }
                        _ => {
                            self.pos = escape_pos;
                            return Err(self.syntax("invalid escape"));
                        }
                    }
                }
                Some(_) => return Err(self.syntax("control character in string")),
                None => return Err(self.syntax("unterminated string")),
            }
        }
    }

    fn number(&mut self) -> PluginResult<Node> {
        let start = self.pos;
        let digits = |p: &mut Self| {
            let from = p.pos;
            while p.peek().is_some_and(|b| b.is_ascii_digit()) {
                p.pos += 1;
            }
            p.pos - from
        };
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                digits(self);
            }
            _ => return Err(self.syntax("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if digits(self) == 0 {
                return Err(self.syntax("invalid number"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if digits(self) == 0 {
                return Err(self.syntax("invalid number"));
            }
        }
        let lexeme = &self.src[start..self.pos];
        let value: f64 = lexeme.parse().map_err(|_| self.syntax("invalid number"))?;
        if !value.is_finite() {
            return Err(self
                .error_at("json.number_range", start)
                .with("number", lexeme.to_owned()));
        }
        if significant(lexeme) != significant(&format_number(value)) {
            self.lossy += 1;
        }
        Ok(Node::Number(value))
    }
}

/// 有效数字（去掉符号、小数点、指数与首尾的 0），用于判断精度是否变化
fn significant(lexeme: &str) -> String {
    let mantissa = lexeme.split(['e', 'E']).next().unwrap_or("");
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    digits
        .trim_start_matches('0')
        .trim_end_matches('0')
        .to_owned()
}

/// ECMAScript Number.prototype.toString 的数字序列化（RFC 8785 §3.2.2.3）
pub fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    // Rust 的 {:e} 输出最短可往返的十进制有效数字，如 1.2345e-7
    let sci = format!("{:e}", value.abs());
    let (mantissa, exponent) = sci.split_once('e').expect("scientific notation");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exponent.parse::<i32>().expect("exponent") + 1;
    let mut out = String::new();
    if value < 0.0 {
        out.push('-');
    }
    if k <= n && n <= 21 {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-n) as usize));
        out.push_str(&digits);
    } else {
        let e = n - 1;
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if e < 0 { '-' } else { '+' });
        out.push_str(&e.abs().to_string());
    }
    out
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write(node: &Node, out: &mut String) {
    match node {
        Node::Null => out.push_str("null"),
        Node::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Node::Number(n) => out.push_str(&format_number(*n)),
        Node::String(s) => write_string(s, out),
        Node::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write(item, out);
            }
            out.push(']');
        }
        Node::Object(members) => {
            // 按 UTF-16 码元排序，而不是按 Unicode 码位或 UTF-8 字节
            let mut sorted: Vec<&(String, Node)> = members.iter().collect();
            sorted.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (index, (key, value)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write(value, out);
            }
            out.push('}');
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Canonical {
    pub text: String,
    /// 精度发生变化的数字个数（binary64 语义）
    pub lossy_numbers: usize,
}

pub fn canonicalize(input: &str) -> PluginResult<Canonical> {
    if input.trim().is_empty() {
        return Err(PluginError::new("json.empty"));
    }
    let mut parser = Parser::new(input);
    let node = parser.value(0)?;
    parser.skip_ws();
    if parser.pos < parser.bytes.len() {
        return Err(parser.syntax("trailing characters"));
    }
    let mut text = String::new();
    write(&node, &mut text);
    Ok(Canonical {
        text,
        lossy_numbers: parser.lossy,
    })
}

#[cfg(test)]
#[path = "jcs_test.rs"]
mod tests;
