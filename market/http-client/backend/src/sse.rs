//! text/event-stream 增量解析（WHATWG HTML「Server-sent events」的解析规则）。
//!
//! 按字节切行（CR、LF、CRLF 都是行结束，CR 落在块末尾时等下一块再判断），
//! 每行再按 UTF-8 解码，所以多字节字符跨块也不会被破坏。

/// 单行超过这个长度时截断，防止没有换行的数据无限占用内存
pub const MAX_LINE: usize = 4 * 1024 * 1024;
/// 单个事件的 data 上限，超出部分丢弃并标记 truncated
pub const MAX_DATA: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// event 字段，未设置时为 None（按规范视为 message）
    pub event: Option<String>,
    /// 派发时的 last event id（会延续到之后的事件）
    pub id: Option<String>,
    pub data: String,
    /// 收到这个事件之前出现过的 retry 值
    pub retry: Option<u64>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Event(Event),
    /// 以冒号开头的注释行（通常是心跳）
    Comment,
}

#[derive(Default)]
pub struct Parser {
    line: Vec<u8>,
    line_truncated: bool,
    /// 上一块以 CR 结尾：下一块开头的 LF 属于同一个换行
    after_cr: bool,
    started: bool,
    event: Option<String>,
    data: String,
    has_data: bool,
    data_truncated: bool,
    last_id: Option<String>,
    retry: Option<u64>,
    retry_changed: bool,
}

impl Parser {
    pub fn last_id(&self) -> Option<&str> {
        self.last_id.as_deref()
    }

    pub fn retry(&self) -> Option<u64> {
        self.retry
    }

    /// 喂入一块字节，返回其中完整结束的事件与注释
    pub fn feed(&mut self, chunk: &[u8], out: &mut Vec<Item>) {
        let mut bytes = chunk;
        if self.after_cr {
            self.after_cr = false;
            if let Some(rest) = bytes.strip_prefix(b"\n") {
                bytes = rest;
            }
        }
        while let Some(pos) = bytes.iter().position(|b| *b == b'\n' || *b == b'\r') {
            self.push(&bytes[..pos]);
            let line = std::mem::take(&mut self.line);
            self.line_truncated = false;
            self.process(&line, out);
            let cr = bytes[pos] == b'\r';
            bytes = &bytes[pos + 1..];
            if cr {
                match bytes.first() {
                    Some(b'\n') => bytes = &bytes[1..],
                    None => self.after_cr = true,
                    _ => {}
                }
            }
        }
        self.push(bytes);
    }

    /// 流结束：按规范，未以空行结束的事件不派发；返回是否丢弃了未完成的内容
    pub fn finish(&mut self) -> bool {
        let pending = !self.line.is_empty() || self.has_data;
        self.line.clear();
        self.reset_event();
        pending
    }

    fn push(&mut self, bytes: &[u8]) {
        let room = MAX_LINE.saturating_sub(self.line.len());
        if bytes.len() > room {
            self.line_truncated = true;
        }
        self.line.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    fn reset_event(&mut self) {
        self.event = None;
        self.data.clear();
        self.has_data = false;
        self.data_truncated = false;
    }

    fn process(&mut self, raw: &[u8], out: &mut Vec<Item>) {
        let mut raw = raw;
        if !self.started {
            self.started = true;
            raw = raw.strip_prefix("\u{feff}".as_bytes()).unwrap_or(raw);
        }
        if raw.is_empty() {
            self.dispatch(out);
            return;
        }
        if raw[0] == b':' {
            out.push(Item::Comment);
            return;
        }
        let line = String::from_utf8_lossy(raw);
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_ref(), ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => {
                self.has_data = true;
                self.append_data(value);
                self.append_data("\n");
                if self.line_truncated {
                    self.data_truncated = true;
                }
            }
            "id" if !value.contains('\0') => self.last_id = Some(value.to_owned()),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                if let Ok(ms) = value.parse() {
                    self.retry = Some(ms);
                    self.retry_changed = true;
                }
            }
            _ => {}
        }
    }

    fn append_data(&mut self, text: &str) {
        let room = MAX_DATA.saturating_sub(self.data.len());
        if text.len() <= room {
            self.data.push_str(text);
            return;
        }
        self.data_truncated = true;
        let mut end = room;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.data.push_str(&text[..end]);
    }

    fn dispatch(&mut self, out: &mut Vec<Item>) {
        if !self.has_data {
            self.reset_event();
            return;
        }
        let mut data = std::mem::take(&mut self.data);
        if data.ends_with('\n') {
            data.pop();
        }
        out.push(Item::Event(Event {
            event: self.event.take().filter(|e| !e.is_empty()),
            id: self.last_id.clone().filter(|id| !id.is_empty()),
            data,
            retry: self.retry_changed.then_some(self.retry).flatten(),
            truncated: self.data_truncated,
        }));
        self.retry_changed = false;
        self.reset_event();
    }
}

/// 非 SSE 的流式响应：按块解码 UTF-8，块末尾不完整的多字节字符留到下一块
#[derive(Default)]
pub struct Utf8Chunks {
    tail: Vec<u8>,
}

impl Utf8Chunks {
    pub fn feed(&mut self, chunk: &[u8]) -> String {
        let mut bytes = std::mem::take(&mut self.tail);
        bytes.extend_from_slice(chunk);
        let keep = incomplete_tail(&bytes);
        self.tail = bytes.split_off(bytes.len() - keep);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    pub fn finish(&mut self) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut self.tail)).into_owned()
    }
}

/// 末尾不完整的 UTF-8 序列长度（最多 3 字节）；非法字节不保留，交给有损解码
fn incomplete_tail(bytes: &[u8]) -> usize {
    for back in 1..=3.min(bytes.len()) {
        let byte = bytes[bytes.len() - back];
        if byte & 0xc0 == 0x80 {
            continue;
        }
        let need = match byte {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return 0,
        };
        return if need > back { back } else { 0 };
    }
    0
}

#[cfg(test)]
#[path = "sse_test.rs"]
mod tests;
