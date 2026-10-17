//! 流式接收：任务打开连接并返回第一批事件，之后由前端短轮询取增量。
//!
//! WASM 实例同一时间只能执行一个调用，所以任务不能一直占着连接；
//! 连接保存在会话里，每次 `stream_poll` 最多读一秒。
//! text/event-stream 按 SSE 解析为事件，其他响应按块显示。保留的事件数量与字节数有上限，超出时丢弃最早的。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};
use tf_plugin_guest::net::{Stream, StreamEvent as NetEvent, StreamRequest};

use crate::http::{self, Request};
use crate::sse::{Item, Parser, Utf8Chunks};

/// 保留的事件条数与 data 字节数上限
pub const MAX_EVENTS: usize = 5000;
pub const MAX_RETAINED: usize = 16 * 1024 * 1024;
/// 每次返回给前端的事件条数上限，其余留到下一次轮询
pub const BATCH: usize = 500;
/// 不超过这个大小的 JSON 数据额外给出缩进后的展示文本
const FORMAT_LIMIT: usize = 64 * 1024;
/// 同时保留的会话数（含已结束、尚未关闭的）
const MAX_SESSIONS: usize = 4;
/// 流式连接没有固定时长，用户手动停止；这里只设一个很长的兜底期限
const DEADLINE_MS: u32 = 24 * 60 * 60 * 1000;
/// 单次读取最长等待；轮询总预算由调用方给出
const READ_SLICE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// text/event-stream：解析为事件
    Sse,
    /// 其他响应：按收到的块显示
    Raw,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Event,
    Chunk,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreamItem {
    pub seq: u64,
    /// 距连接建立的毫秒数
    pub at_ms: u64,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub data: String,
    /// data 是 JSON 对象或数组时的缩进展示；原始 data 保持不变
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<u64>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EndReason {
    /// 服务端正常结束
    Ended,
    /// 用户停止
    Stopped,
    /// 网络错误或超时
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct End {
    pub reason: EndReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<PluginError>,
    /// 结束时还有未以空行结束的事件，按规范被丢弃
    pub discarded: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub events: Vec<StreamItem>,
    /// 至今收到的事件总数（含已丢弃的）
    pub total: u64,
    /// 因超出上限而丢弃的最早事件数
    pub dropped: u64,
    pub bytes: u64,
    /// 注释行（心跳）数
    pub comments: u64,
    pub retry: Option<u64>,
    pub last_id: Option<String>,
    pub elapsed_ms: u64,
    pub done: Option<End>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub session: u64,
    pub status: u16,
    pub status_text: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub content_type: Option<String>,
    pub mode: Mode,
    #[serde(flatten)]
    pub batch: Batch,
}

struct Session {
    stream: Option<Stream>,
    mode: Mode,
    status: u16,
    parser: Parser,
    chunks: Utf8Chunks,
    events: VecDeque<StreamItem>,
    retained: usize,
    /// 已经交给前端的最大序号
    delivered: u64,
    next_seq: u64,
    dropped: u64,
    bytes: u64,
    comments: u64,
    started: Instant,
    done: Option<End>,
    /// 用了机密变量：错误参数里可能带出地址或凭据，统一隐去
    redact: bool,
}

impl Session {
    fn new(stream: Stream, mode: Mode, redact: bool) -> Self {
        Self {
            status: stream.head().status,
            stream: Some(stream),
            mode,
            parser: Parser::default(),
            chunks: Utf8Chunks::default(),
            events: VecDeque::new(),
            retained: 0,
            delivered: 0,
            next_seq: 0,
            dropped: 0,
            bytes: 0,
            comments: 0,
            started: Instant::now(),
            done: None,
            redact,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn undelivered(&self) -> u64 {
        self.next_seq - self.delivered
    }

    fn push(
        &mut self,
        kind: Kind,
        event: Option<String>,
        id: Option<String>,
        data: String,
        retry: Option<u64>,
        truncated: bool,
    ) {
        self.next_seq += 1;
        let formatted = if kind == Kind::Event {
            format_json(&data)
        } else {
            None
        };
        self.retained += data.len() + formatted.as_ref().map_or(0, String::len);
        self.events.push_back(StreamItem {
            seq: self.next_seq,
            at_ms: self.elapsed_ms(),
            kind,
            event,
            id,
            data,
            formatted,
            retry,
            truncated,
        });
        while self.events.len() > MAX_EVENTS || self.retained > MAX_RETAINED {
            let Some(old) = self.events.pop_front() else {
                break;
            };
            self.retained -= old.data.len() + old.formatted.as_ref().map_or(0, String::len);
            self.dropped += 1;
        }
    }

    fn ingest(&mut self, data: &[u8]) {
        self.bytes += data.len() as u64;
        match self.mode {
            Mode::Sse => {
                let mut items = Vec::new();
                self.parser.feed(data, &mut items);
                for item in items {
                    match item {
                        Item::Comment => self.comments += 1,
                        Item::Event(e) => {
                            self.push(Kind::Event, e.event, e.id, e.data, e.retry, e.truncated)
                        }
                    }
                }
            }
            Mode::Raw => {
                let text = self.chunks.feed(data);
                if !text.is_empty() {
                    self.push(Kind::Chunk, None, None, text, None, false);
                }
            }
        }
    }

    fn finish(&mut self, reason: EndReason, error: Option<PluginError>) {
        if self.done.is_some() {
            return;
        }
        self.stream = None;
        let discarded = match self.mode {
            Mode::Sse => self.parser.finish(),
            Mode::Raw => {
                let rest = self.chunks.finish();
                if !rest.is_empty() {
                    self.push(Kind::Chunk, None, None, rest, None, false);
                }
                false
            }
        };
        let error = error.map(|e| {
            let e = http::network_error(e);
            if self.redact { redact(e) } else { e }
        });
        self.done = Some(End {
            reason,
            error,
            discarded,
        });
    }

    /// 读取直到预算用完、攒够一批，或者有新事件且暂时没有更多数据
    fn pump(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while self.done.is_none() && self.undelivered() < BATCH as u64 {
            let Some(stream) = self.stream.as_mut() else {
                break;
            };
            let left = deadline.saturating_duration_since(Instant::now());
            match stream.read(left.min(READ_SLICE)) {
                Ok(NetEvent::Data(data)) => self.ingest(&data),
                Ok(NetEvent::Pending) => {
                    if left.is_zero() || self.undelivered() > 0 {
                        break;
                    }
                }
                Ok(NetEvent::End(_)) => self.finish(EndReason::Ended, None),
                Err(err) => self.finish(EndReason::Failed, Some(err)),
            }
        }
    }

    fn batch(&mut self) -> Batch {
        let events: Vec<StreamItem> = self
            .events
            .iter()
            .filter(|e| e.seq > self.delivered)
            .take(BATCH)
            .cloned()
            .collect();
        if let Some(last) = events.last() {
            self.delivered = last.seq;
        }
        // 丢弃的事件里可能有还没交出去的
        self.delivered = self
            .delivered
            .max(self.events.front().map_or(self.next_seq, |e| e.seq - 1));
        Batch {
            events,
            total: self.next_seq,
            dropped: self.dropped,
            bytes: self.bytes,
            comments: self.comments,
            retry: self.parser.retry(),
            last_id: self
                .parser
                .last_id()
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            elapsed_ms: self.elapsed_ms(),
            done: self.done.clone(),
        }
    }
}

fn format_json(data: &str) -> Option<String> {
    let trimmed = data.trim_start();
    if data.len() > FORMAT_LIMIT || !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    serde_json::to_string_pretty(&value)
        .ok()
        .filter(|pretty| pretty != data)
}

fn redact(mut error: PluginError) -> PluginError {
    for value in error.params.values_mut().filter(|v| v.is_string()) {
        *value = serde_json::Value::String("[redacted]".into());
    }
    error
}

fn is_event_stream(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|ct| {
        ct.split(';')
            .next()
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"))
    })
}

#[derive(Default)]
pub struct Streams {
    sessions: Mutex<(u64, BTreeMap<u64, Session>)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionArgs {
    pub session: u64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Sse,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub session: u64,
    pub format: ExportFormat,
}

/// 首批事件最多等待的时间
const FIRST_BATCH: Duration = Duration::from_millis(1000);
/// 每次轮询最多等待的时间
pub const POLL: Duration = Duration::from_millis(1000);

impl Streams {
    fn lock(&self) -> std::sync::MutexGuard<'_, (u64, BTreeMap<u64, Session>)> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 打开流：补上事件流需要的请求头，读取响应头与第一批事件
    pub fn open(
        &self,
        request: &Request,
        last_event_id: Option<&str>,
        redact_urls: bool,
        ctx: &dyn TaskContext,
    ) -> PluginResult<Opened> {
        let mut parts = http::outgoing(request)?;
        if !http::has_header(request, "accept") {
            parts
                .headers
                .push(("Accept".into(), "text/event-stream".into()));
        }
        if !http::has_header(request, "cache-control") {
            parts
                .headers
                .push(("Cache-Control".into(), "no-cache".into()));
        }
        if let Some(id) = last_event_id.filter(|id| !id.is_empty())
            && !http::has_header(request, "last-event-id")
        {
            parts.headers.push(("Last-Event-ID".into(), id.to_owned()));
        }
        http::log_request(ctx, &parts, redact_urls);
        let outgoing = StreamRequest {
            method: parts.method,
            url: parts.url.to_string(),
            headers: parts.headers,
            body: parts.body,
            timeout_ms: DEADLINE_MS,
            http2_prior_knowledge: false,
        };
        let stream = Stream::open(&outgoing, Some(ctx)).map_err(http::network_error)?;
        let head = stream.head().clone();
        ctx.log(
            LogLevel::Info,
            "http.stream_open",
            json!({ "status": head.status }),
        );
        let content_type = head
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());
        let mode = if is_event_stream(content_type.as_deref()) {
            Mode::Sse
        } else {
            Mode::Raw
        };
        let mut session = Session::new(stream, mode, redact_urls);
        session.pump(FIRST_BATCH);
        let batch = session.batch();
        let mut guard = self.lock();
        let (next, sessions) = &mut *guard;
        *next += 1;
        let id = *next;
        sessions.insert(id, session);
        while sessions.len() > MAX_SESSIONS {
            sessions.pop_first();
        }
        Ok(Opened {
            session: id,
            status: head.status,
            status_text: head.reason,
            version: head.version,
            headers: head.headers,
            content_type,
            mode,
            batch,
        })
    }

    fn with<T>(&self, id: u64, f: impl FnOnce(&mut Session) -> T) -> PluginResult<T> {
        let mut guard = self.lock();
        let session = guard
            .1
            .get_mut(&id)
            .ok_or_else(|| PluginError::new("http.stream_expired"))?;
        Ok(f(session))
    }

    pub fn poll(&self, id: u64, budget: Duration) -> PluginResult<Batch> {
        self.with(id, |s| {
            s.pump(budget);
            s.batch()
        })
    }

    pub fn stop(&self, id: u64) -> PluginResult<Batch> {
        self.with(id, |s| {
            s.finish(EndReason::Stopped, None);
            s.batch()
        })
    }

    pub fn close(&self, id: u64) {
        self.lock().1.remove(&id);
    }

    /// 导出保留的事件：JSON（逐条，含时间与序号）或还原为 text/event-stream 文本
    pub fn export(&self, id: u64, format: ExportFormat) -> PluginResult<String> {
        self.with(id, |s| match format {
            ExportFormat::Json => serde_json::to_string_pretty(&json!({
                "status": s.status,
                "mode": s.mode,
                "total": s.next_seq,
                "dropped": s.dropped,
                "bytes": s.bytes,
                "done": s.done,
                "events": s.events,
            }))
            .unwrap_or_default(),
            ExportFormat::Sse => s.events.iter().map(|e| to_sse(e, s.mode)).collect(),
        })
    }
}

/// 把一条记录还原为事件流文本；分块响应原样拼接
fn to_sse(item: &StreamItem, mode: Mode) -> String {
    if mode == Mode::Raw {
        return item.data.clone();
    }
    let mut out = String::new();
    if let Some(event) = &item.event {
        out.push_str(&format!("event: {event}\n"));
    }
    if let Some(id) = &item.id {
        out.push_str(&format!("id: {id}\n"));
    }
    if let Some(retry) = item.retry {
        out.push_str(&format!("retry: {retry}\n"));
    }
    for line in item.data.split('\n') {
        out.push_str(&format!("data: {line}\n"));
    }
    out.push('\n');
    out
}

#[cfg(test)]
#[path = "stream_test.rs"]
mod tests;
