//! 位置扫描：对已通过语法校验的 JSON 文本做一次轻量结构扫描，
//! 记录每个值（RFC 6901 JSON Pointer → 起始行列）的位置，供诊断定位原文。
//! 语法本身以 serde_json 的解析为准，这里只在结构上跟随，出错时静默放弃剩余部分。

use std::collections::HashMap;

/// 输入大小上限
pub const MAX_INPUT: usize = 8 * 1024 * 1024;
/// 节点深度上限
pub const MAX_DEPTH: usize = 64;

pub type Span = (u64, u64);

/// pointer → 该值起始的（行, 列），均为 1 开始
pub type Spans = HashMap<String, Span>;

#[derive(Debug)]
pub struct ScanLimit {
    pub kind: &'static str,
}

struct Scanner<'a> {
    chars: std::str::Chars<'a>,
    peeked: Option<char>,
    line: u64,
    column: u64,
    spans: Spans,
    depth: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars(),
            peeked: None,
            line: 1,
            column: 0,
            spans: Spans::new(),
            depth: 0,
        }
    }

    fn bump(&mut self) -> Option<char> {
        let next = self.peeked.take().or_else(|| self.chars.next())?;
        if next == '\n' {
            self.line += 1;
            self.column = 0;
        } else {
            self.column += 1;
        }
        Some(next)
    }

    fn peek(&mut self) -> Option<char> {
        if self.peeked.is_none() {
            self.peeked = self.chars.next();
        }
        self.peeked
    }

    /// 跳过空白与可能的逗号/冒号等分隔符由调用方处理，这里只跳空白
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.bump();
        }
    }

    fn expect(&mut self, expected: char) -> bool {
        match self.peek() {
            Some(c) if c == expected => {
                self.bump();
                true
            }
            _ => false,
        }
    }

    /// 跳过一个 JSON 值的结构（字符串逐字符处理转义）
    fn skip_value(&mut self) -> Result<(), ScanLimit> {
        self.skip_ws();
        match self.peek() {
            Some('"') => self.skip_string(),
            Some('{') => self.skip_object(),
            Some('[') => self.skip_array(),
            Some(_) => {
                // number / true / false / null：读到结构字符为止
                while matches!(
                    self.peek(),
                    Some(c) if !matches!(c, ',' | '}' | ']' | ' ' | '\t' | '\n' | '\r')
                ) {
                    self.bump();
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn skip_string(&mut self) -> Result<(), ScanLimit> {
        if !self.expect('"') {
            return Ok(());
        }
        while let Some(c) = self.bump() {
            match c {
                '"' => return Ok(()),
                '\\' => {
                    self.bump();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn skip_array(&mut self) -> Result<(), ScanLimit> {
        self.expect('[');
        loop {
            self.skip_ws();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    return Ok(());
                }
                None => return Ok(()),
                _ => {}
            }
            self.skip_value()?;
            self.skip_ws();
            if let Some(',') = self.peek() {
                self.bump();
            }
        }
    }

    fn skip_object(&mut self) -> Result<(), ScanLimit> {
        self.expect('{');
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') => {
                    self.bump();
                    return Ok(());
                }
                None => return Ok(()),
                _ => {}
            }
            self.skip_string()?;
            self.skip_ws();
            self.expect(':');
            self.skip_value()?;
            self.skip_ws();
            if let Some(',') = self.peek() {
                self.bump();
            }
        }
    }

    /// 结构化扫描：记录每个值的起始位置后跳过它
    fn record_value(&mut self, pointer: &str) -> Result<(), ScanLimit> {
        self.skip_ws();
        let span = (self.line, self.column + 1);
        match self.peek() {
            Some('[') => {
                self.spans.insert(pointer.to_owned(), span);
                self.walk_array(pointer)
            }
            Some('{') => {
                self.spans.insert(pointer.to_owned(), span);
                self.walk_object(pointer)
            }
            Some(_) => {
                self.spans.insert(pointer.to_owned(), span);
                self.skip_value()
            }
            None => Ok(()),
        }
    }

    fn walk_array(&mut self, pointer: &str) -> Result<(), ScanLimit> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ScanLimit { kind: "depth" });
        }
        self.expect('[');
        let mut index = 0usize;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    self.depth -= 1;
                    return Ok(());
                }
                None => {
                    self.depth -= 1;
                    return Ok(());
                }
                _ => {}
            }
            let child = format!("{pointer}/{index}");
            self.record_value(&child)?;
            index += 1;
            self.skip_ws();
            if !self.expect(',') {
                // 容错跟随：期望 ']'，交给 serde_json 的语法校验报告
                self.expect(']');
                self.depth -= 1;
                return Ok(());
            }
        }
    }

    fn walk_object(&mut self, pointer: &str) -> Result<(), ScanLimit> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ScanLimit { kind: "depth" });
        }
        self.expect('{');
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') => {
                    self.bump();
                    self.depth -= 1;
                    return Ok(());
                }
                None => {
                    self.depth -= 1;
                    return Ok(());
                }
                _ => {}
            }
            let key = self.read_key();
            self.skip_ws();
            self.expect(':');
            let child = format!("{pointer}/{}", escape_token(&key));
            self.record_value(&child)?;
            self.skip_ws();
            if !self.expect(',') {
                self.expect('}');
                self.depth -= 1;
                return Ok(());
            }
        }
    }

    /// 读取成员键的原始内容（不含引号）
    fn read_key(&mut self) -> String {
        if !self.expect('"') {
            return String::new();
        }
        let mut key = String::new();
        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => {
                    // 键里的转义按字面保留，仅处理最常见的引号与反斜杠
                    match self.peek() {
                        Some('"') => {
                            self.bump();
                            key.push('"');
                        }
                        Some('\\') => {
                            self.bump();
                            key.push('\\');
                        }
                        _ => key.push(c),
                    }
                }
                _ => key.push(c),
            }
        }
        key
    }
}

/// RFC 6901：键中的 ~ 与 / 需要转义
fn escape_token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// 扫描文本，返回 pointer → 行列；文本超限返回错误
pub fn scan(text: &str) -> Result<Spans, ScanLimit> {
    if text.len() > MAX_INPUT {
        return Err(ScanLimit { kind: "size" });
    }
    let mut scanner = Scanner::new(text);
    scanner.record_value("")?;
    Ok(scanner.spans)
}

/// 查询某个 pointer 的原文位置
pub fn locate(spans: &Spans, pointer: &str) -> Option<Span> {
    spans.get(pointer).copied()
}

#[cfg(test)]
#[path = "spans_test.rs"]
mod tests;
