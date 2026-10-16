//! Conservative JSON repair. The parser never evaluates input or guesses missing values.
//! Numbers retain their decimal precision under serde_json's arbitrary_precision feature.

use std::fmt::Write;

use serde::Serialize;
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

const MAX_BYTES: usize = 2 * 1024 * 1024;
// serde_json's default limit rejects its 128th nested container. Match strict
// parsing so repaired output remains usable by the existing Format action.
const MAX_DEPTH: usize = 127;
const MAX_NODES: usize = 100_000;

#[derive(Debug, Serialize, PartialEq)]
pub struct Repair {
    pub code: &'static str,
    pub count: usize,
}

#[derive(Debug)]
pub struct Repaired {
    pub value: Value,
    pub repairs: Vec<Repair>,
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
    nodes: usize,
    repairs: Vec<Repair>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }

    fn mark(&mut self, code: &'static str) {
        if let Some(repair) = self.repairs.iter_mut().find(|repair| repair.code == code) {
            repair.count += 1;
        } else {
            self.repairs.push(Repair { code, count: 1 });
        }
    }

    fn error_at(&self, code: &'static str, pos: usize) -> PluginError {
        let before = &self.source[..self.source.floor_char_boundary(pos)];
        let mut line = 1;
        let mut column = 1;
        let mut previous_cr = false;
        for ch in before.chars() {
            match ch {
                '\r' => {
                    line += 1;
                    column = 1;
                }
                '\n' => {
                    if !previous_cr {
                        line += 1;
                    }
                    column = 1;
                }
                _ => column += ch.len_utf16(),
            }
            previous_cr = ch == '\r';
        }
        PluginError::new(code)
            .with("line", line)
            .with("column", column)
    }

    fn syntax(&self, reason: &'static str) -> PluginError {
        self.error_at("json.repair_syntax", self.pos)
            .with("reason", reason)
    }

    fn skip(&mut self) -> PluginResult<()> {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.pos += 1;
            }
            let rest = &self.source[self.pos..];
            if rest.starts_with("//") {
                let end = rest
                    .char_indices()
                    .find(|(_, ch)| matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
                    .map_or(rest.len(), |(index, ch)| index + ch.len_utf8());
                self.pos += end;
                self.mark("json.repair.comments");
            } else if let Some(comment) = rest.strip_prefix("/*") {
                let end = comment
                    .find("*/")
                    .ok_or_else(|| self.syntax("unclosed_comment"))?;
                self.pos += end + 4;
                self.mark("json.repair.comments");
            } else {
                return Ok(());
            }
        }
    }

    fn value(&mut self, depth: usize) -> PluginResult<Value> {
        self.skip()?;
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(self
                .error_at("json.repair_too_many_nodes", self.pos)
                .with("max", MAX_NODES));
        }
        match self.peek() {
            Some(b'{' | b'[') if depth >= MAX_DEPTH => Err(self
                .error_at("json.repair_too_deep", self.pos)
                .with("max", MAX_DEPTH)),
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'\'' | b'"') => self.string().map(Value::String),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$') => self.literal(),
            _ => Err(self.syntax("expected_value")),
        }
    }

    /// Whitespace/comments are required between word/number tokens: `1true` is
    /// ambiguous, while `1 true` and `1{...}` have recognizable value boundaries.
    fn atom_boundary(&self) -> bool {
        match self.peek() {
            None => true,
            Some(b',' | b']' | b'}' | b'{' | b'[' | b'\'' | b'"') => true,
            Some(b'/') => {
                self.source[self.pos..].starts_with("//")
                    || self.source[self.pos..].starts_with("/*")
            }
            Some(b) => b.is_ascii_whitespace(),
        }
    }

    fn number(&mut self) -> PluginResult<Value> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.syntax("invalid_number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let digits = self.pos;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == digits {
                return Err(self.syntax("invalid_number"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            let digits = self.pos;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == digits {
                return Err(self.syntax("invalid_number"));
            }
        }
        if !self.atom_boundary() {
            return Err(self.syntax("ambiguous_number"));
        }
        serde_json::from_str(&self.source[start..self.pos])
            .map_err(|_| self.syntax("invalid_number"))
    }

    fn literal(&mut self) -> PluginResult<Value> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
        {
            self.pos += 1;
        }
        if !self.atom_boundary() {
            return Err(self.syntax("ambiguous_literal"));
        }
        match &self.source[start..self.pos] {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            "null" => Ok(Value::Null),
            "True" => {
                self.mark("json.repair.python_literals");
                Ok(Value::Bool(true))
            }
            "False" => {
                self.mark("json.repair.python_literals");
                Ok(Value::Bool(false))
            }
            "None" => {
                self.mark("json.repair.python_literals");
                Ok(Value::Null)
            }
            "undefined" => {
                self.mark("json.repair.undefined");
                Ok(Value::Null)
            }
            _ => Err(self.syntax("unknown_literal")),
        }
    }

    fn string(&mut self) -> PluginResult<String> {
        let quote = self.peek().expect("caller checked quote");
        self.pos += 1;
        let mut json = String::from("\"");
        loop {
            match self.peek() {
                Some(b) if b == quote => {
                    self.pos += 1;
                    json.push('"');
                    if quote == b'\'' {
                        self.mark("json.repair.single_quotes");
                    }
                    return serde_json::from_str(&json).map_err(|_| self.syntax("invalid_string"));
                }
                Some(b'\\') => self.escape(&mut json)?,
                Some(b) if b < 0x20 => {
                    let _ = write!(json, "\\u{b:04x}");
                    self.pos += 1;
                    self.mark("json.repair.control_characters");
                }
                Some(b'"') => {
                    // Only reached in a single-quoted string.
                    json.push_str("\\\"");
                    self.pos += 1;
                }
                Some(_) => {
                    let ch = self.source[self.pos..].chars().next().expect("character");
                    json.push(ch);
                    self.pos += ch.len_utf8();
                }
                None => return Err(self.syntax("unclosed_string")),
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> PluginResult<()> {
        self.pos += 1;
        let byte = self.peek().ok_or_else(|| self.syntax("invalid_escape"))?;
        self.pos += 1;
        match byte {
            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                out.push('\\');
                out.push(char::from(byte));
                return Ok(());
            }
            b'u' => {
                let digits = self.hex(4)?;
                out.push_str("\\u");
                out.push_str(digits);
                return Ok(());
            }
            b'\'' => out.push('\''),
            b'x' => {
                let digits = self.hex(2)?;
                out.push_str("\\u00");
                out.push_str(digits);
            }
            b'v' => out.push_str("\\u000b"),
            b'0' if !self.peek().is_some_and(|b| b.is_ascii_digit()) => {
                out.push_str("\\u0000");
            }
            b'\n' => {}
            b'\r' => {
                if self.peek() == Some(b'\n') {
                    self.pos += 1;
                }
            }
            _ => return Err(self.syntax("invalid_escape")),
        }
        self.mark("json.repair.string_escapes");
        Ok(())
    }

    fn hex(&mut self, count: usize) -> PluginResult<&'a str> {
        let digits = self
            .source
            .get(self.pos..self.pos + count)
            .filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| self.syntax("invalid_escape"))?;
        self.pos += count;
        Ok(digits)
    }

    fn key(&mut self) -> PluginResult<String> {
        if matches!(self.peek(), Some(b'\'' | b'"')) {
            return self.string();
        }
        let start = self.pos;
        for (index, ch) in self.source[start..].char_indices() {
            let allowed = if index == 0 {
                ch == '_' || ch == '$' || ch.is_alphabetic()
            } else {
                ch == '_' || ch == '$' || ch.is_alphanumeric()
            };
            if !allowed {
                break;
            }
            self.pos = start + index + ch.len_utf8();
        }
        if self.pos == start {
            return Err(self.syntax("expected_key"));
        }
        let key = self.source[start..self.pos].to_owned();
        self.mark("json.repair.unquoted_keys");
        Ok(key)
    }

    fn extra_commas(&mut self) -> PluginResult<()> {
        while self.peek() == Some(b',') {
            self.pos += 1;
            self.mark("json.repair.extra_commas");
            self.skip()?;
        }
        Ok(())
    }

    fn object(&mut self, depth: usize) -> PluginResult<Value> {
        self.pos += 1;
        let mut members = Map::new();
        self.skip()?;
        self.extra_commas()?;
        loop {
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(members));
                }
                None if !members.is_empty() => {
                    self.mark("json.repair.unclosed_containers");
                    return Ok(Value::Object(members));
                }
                None => return Err(self.syntax("incomplete_container")),
                _ => {}
            }
            let key_pos = self.pos;
            let key = self.key()?;
            if members.contains_key(&key) {
                return Err(self
                    .error_at("json.repair_duplicate_key", key_pos)
                    .with("key", key));
            }
            self.skip()?;
            if self.peek() != Some(b':') {
                return Err(self.syntax("expected_colon"));
            }
            self.pos += 1;
            let value = self.value(depth)?;
            members.insert(key, value);
            self.skip()?;
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip()?;
                    self.extra_commas()?;
                    if self.peek() == Some(b'}') {
                        self.mark("json.repair.extra_commas");
                    }
                    if self.peek().is_none() {
                        return Err(self.syntax("missing_member"));
                    }
                }
                Some(b'}') | None => {}
                Some(b'\'' | b'"' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$') => {
                    self.mark("json.repair.missing_commas");
                }
                Some(b) if b >= 0x80 => {
                    self.mark("json.repair.missing_commas");
                }
                _ => return Err(self.syntax("expected_object_separator")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> PluginResult<Value> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip()?;
        loop {
            match self.peek() {
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                None if !items.is_empty() => {
                    self.mark("json.repair.unclosed_containers");
                    return Ok(Value::Array(items));
                }
                None => return Err(self.syntax("incomplete_container")),
                _ => {}
            }
            items.push(self.value(depth)?);
            self.skip()?;
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip()?;
                    if self.peek() == Some(b']') {
                        self.mark("json.repair.extra_commas");
                    }
                    if matches!(self.peek(), None | Some(b',')) {
                        return Err(self.syntax("missing_value"));
                    }
                }
                Some(b']') | None => {}
                Some(
                    b'{'
                    | b'['
                    | b'\''
                    | b'"'
                    | b'-'
                    | b'0'..=b'9'
                    | b'a'..=b'z'
                    | b'A'..=b'Z'
                    | b'_'
                    | b'$',
                ) => {
                    self.mark("json.repair.missing_commas");
                }
                _ => return Err(self.syntax("expected_array_separator")),
            }
        }
    }
}

pub fn repair(input: &str) -> PluginResult<Repaired> {
    if input.len() > MAX_BYTES {
        return Err(PluginError::new("json.repair_too_large").with("max", MAX_BYTES));
    }
    if input.trim().is_empty() {
        return Err(PluginError::new("json.empty"));
    }
    let mut parser = Parser {
        source: input,
        pos: 0,
        nodes: 0,
        repairs: Vec::new(),
    };
    if input.starts_with('\u{feff}') {
        parser.pos = '\u{feff}'.len_utf8();
        parser.mark("json.repair.bom");
    }
    let value = parser.value(0)?;
    parser.skip()?;
    if parser.pos != input.len() {
        return Err(parser.syntax("trailing_content"));
    }
    Ok(Repaired {
        value,
        repairs: parser.repairs,
    })
}

#[cfg(test)]
#[path = "repair_test.rs"]
mod tests;
