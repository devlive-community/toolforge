//! 会话：保存消息记录与待发送的消息，供连接任务与前端轮询共享。

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;

/// 每个会话最多保留的消息数
const MAX_MESSAGES: usize = 5000;
/// 文本消息最多保存的字符数
pub const MAX_TEXT: usize = 1024 * 1024;
/// 二进制消息保存 base64 的上限
const MAX_BINARY: usize = 256 * 1024;
const HEX_PREVIEW: usize = 256;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    In,
    Out,
    /// 连接、断开、错误等事件
    Event,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub seq: u64,
    pub ts: i64,
    pub direction: Direction,
    /// text / binary / ping / pong / close / open / error
    pub kind: &'static str,
    pub size: usize,
    pub text: Option<String>,
    pub truncated: bool,
    /// 二进制内容的十六进制预览
    pub hex: Option<String>,
    /// 完整二进制内容（不超过 256 KB）
    pub base64: Option<String>,
    /// 事件的错误码或关闭码
    pub code: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Connecting,
    Open,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub url: String,
    pub address: Option<String>,
    pub status: Option<u16>,
    pub protocol: Option<String>,
    pub headers: Vec<(String, String)>,
    pub handshake_ms: Option<u64>,
}

pub enum Outbound {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Close(u16, String),
}

#[derive(Default)]
struct Inner {
    messages: VecDeque<Message>,
    next_seq: u64,
    state: State,
    info: Info,
    outbound: VecDeque<Outbound>,
}

#[derive(Default)]
pub struct Session(Mutex<Inner>);

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn hex_preview(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(HEX_PREVIEW.min(bytes.len()) * 3);
    for (index, byte) in bytes.iter().take(HEX_PREVIEW).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

impl Session {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn push(&self, direction: Direction, kind: &'static str, fill: impl FnOnce(&mut Message)) {
        let mut inner = self.lock();
        inner.next_seq += 1;
        let mut message = Message {
            seq: inner.next_seq,
            ts: now_ms(),
            direction,
            kind,
            size: 0,
            text: None,
            truncated: false,
            hex: None,
            base64: None,
            code: None,
        };
        fill(&mut message);
        inner.messages.push_back(message);
        while inner.messages.len() > MAX_MESSAGES {
            inner.messages.pop_front();
        }
    }

    pub fn text(&self, direction: Direction, text: &str) {
        self.push(direction, "text", |m| {
            m.size = text.len();
            let cut = text.char_indices().nth(MAX_TEXT).map(|(i, _)| i);
            m.truncated = cut.is_some();
            m.text = Some(cut.map_or(text, |i| &text[..i]).to_owned());
        });
    }

    pub fn binary(&self, direction: Direction, kind: &'static str, bytes: &[u8]) {
        self.push(direction, kind, |m| {
            m.size = bytes.len();
            m.hex = Some(hex_preview(bytes));
            m.truncated = bytes.len() > HEX_PREVIEW;
            if bytes.len() <= MAX_BINARY {
                m.base64 = Some(data_encoding::BASE64.encode(bytes));
            }
            // 控制帧的负载通常是可读文本
            if kind != "binary"
                && let Ok(text) = std::str::from_utf8(bytes)
            {
                m.text = Some(text.to_owned());
            }
        });
    }

    pub fn event(&self, kind: &'static str, code: Option<String>, text: Option<String>) {
        self.push(Direction::Event, kind, |m| {
            m.code = code;
            m.text = text;
        });
    }

    pub fn set_state(&self, state: State) {
        self.lock().state = state;
    }

    pub fn state(&self) -> State {
        self.lock().state
    }

    pub fn set_info(&self, info: Info) {
        self.lock().info = info;
    }

    pub fn enqueue(&self, message: Outbound) {
        self.lock().outbound.push_back(message);
    }

    pub fn take_outbound(&self) -> Vec<Outbound> {
        self.lock().outbound.drain(..).collect()
    }

    pub fn clear(&self) {
        self.lock().messages.clear();
    }

    /// 序号大于 after 的消息
    pub fn since(&self, after: u64) -> (Vec<Message>, State, Info, u64) {
        let inner = self.lock();
        let messages = inner
            .messages
            .iter()
            .filter(|m| m.seq > after)
            .cloned()
            .collect();
        (messages, inner.state, inner.info.clone(), inner.next_seq)
    }
}
