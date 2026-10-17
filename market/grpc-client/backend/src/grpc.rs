//! gRPC over HTTP/2：请求帧、请求头与 metadata、响应帧解析、status 与 trailers。
//!
//! 由宿主打开流式请求（明文用 h2c，TLS 由 ALPN 协商 HTTP/2），插件解析长度前缀的消息帧。
//! 调用开始后保存在插件内，界面用短时间的 `poll` 逐批取回消息，实现服务端流的增量展示。

use std::time::{Duration, Instant};

use data_encoding::BASE64;
use prost_reflect::MessageDescriptor;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};
use tf_plugin_guest::net::{Stream, StreamEvent, StreamRequest};

use crate::message;

/// 默认截止时间
pub const DEFAULT_DEADLINE_MS: u64 = 30_000;
const MAX_DEADLINE_MS: u64 = 3_600_000;
/// 截止时间之外留给网络的余量
const GRACE_MS: u64 = 2_000;
pub const DEFAULT_MAX_MESSAGES: usize = 1000;
pub const DEFAULT_MAX_BYTES: usize = 16 * 1024 * 1024;
/// 单条消息上限
const MAX_MESSAGE: usize = 32 * 1024 * 1024;

pub const STATUS_NAMES: [&str; 17] = [
    "OK",
    "CANCELLED",
    "UNKNOWN",
    "INVALID_ARGUMENT",
    "DEADLINE_EXCEEDED",
    "NOT_FOUND",
    "ALREADY_EXISTS",
    "PERMISSION_DENIED",
    "RESOURCE_EXHAUSTED",
    "FAILED_PRECONDITION",
    "ABORTED",
    "OUT_OF_RANGE",
    "UNIMPLEMENTED",
    "INTERNAL",
    "UNAVAILABLE",
    "DATA_LOSS",
    "UNAUTHENTICATED",
];

pub fn status_name(code: u32) -> &'static str {
    STATUS_NAMES
        .get(code as usize)
        .copied()
        .unwrap_or("UNKNOWN")
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// 凭据：只在当前会话保留，保存、日志与导出时都不出现
    #[serde(default)]
    pub secret: bool,
}

fn enabled() -> bool {
    true
}

/// 连接目标：`host:port`，或带 http:// / https:// 前缀
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub address: String,
    #[serde(default)]
    pub tls: bool,
}

impl Target {
    /// 生成基础地址；地址里写了协议时以协议为准
    pub fn base(&self) -> PluginResult<String> {
        let address = self.address.trim().trim_end_matches('/');
        if address.is_empty() {
            return Err(PluginError::new("grpc.empty_target"));
        }
        let base = if address.contains("://") {
            address.to_owned()
        } else {
            format!("{}://{address}", if self.tls { "https" } else { "http" })
        };
        let url = url::Url::parse(&base)
            .map_err(|e| PluginError::new("grpc.invalid_target").with("detail", e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(PluginError::new("grpc.invalid_target").with("detail", base));
        }
        Ok(base)
    }
}

/// gRPC 与 HTTP/2 保留的请求头不能作为 metadata 发送
fn reserved(key: &str) -> bool {
    key.starts_with(':')
        || key.starts_with("grpc-")
        || matches!(
            key,
            "content-type" | "te" | "user-agent" | "host" | "connection" | "transfer-encoding"
        )
}

/// 校验并整理 metadata：键小写；`-bin` 结尾的值必须是 base64
pub fn metadata_headers(entries: &[MetadataEntry]) -> PluginResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for entry in entries
        .iter()
        .filter(|e| e.enabled && !e.key.trim().is_empty())
    {
        let key = entry.key.trim().to_ascii_lowercase();
        let valid_key = key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b));
        if !valid_key {
            return Err(PluginError::new("grpc.metadata_key").with("key", key));
        }
        if reserved(&key) {
            return Err(PluginError::new("grpc.metadata_reserved").with("key", key));
        }
        let value = entry.value.trim();
        if key.ends_with("-bin") {
            BASE64
                .decode(value.as_bytes())
                .or_else(|_| data_encoding::BASE64_NOPAD.decode(value.as_bytes()))
                .map_err(|_| PluginError::new("grpc.metadata_binary").with("key", key.as_str()))?;
        } else if !value.bytes().all(|b| (0x20..0x7f).contains(&b)) {
            return Err(PluginError::new("grpc.metadata_value").with("key", key));
        }
        out.push((key, value.to_owned()));
    }
    Ok(out)
}

/// 长度前缀消息帧：1 字节压缩标志 + 4 字节大端长度 + 消息
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 5);
    out.push(0);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// 从缓冲区中取出完整的消息帧
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn next(&mut self) -> PluginResult<Option<Vec<u8>>> {
        if self.buffer.len() < 5 {
            return Ok(None);
        }
        let compressed = self.buffer[0];
        let len = u32::from_be_bytes([
            self.buffer[1],
            self.buffer[2],
            self.buffer[3],
            self.buffer[4],
        ]) as usize;
        if compressed != 0 {
            return Err(PluginError::new("grpc.compressed"));
        }
        if len > MAX_MESSAGE {
            return Err(PluginError::new("grpc.message_too_large").with("limit", MAX_MESSAGE));
        }
        if self.buffer.len() < 5 + len {
            return Ok(None);
        }
        let payload = self.buffer[5..5 + len].to_vec();
        self.buffer.drain(..5 + len);
        Ok(Some(payload))
    }

    pub fn has_partial(&self) -> bool {
        !self.buffer.is_empty()
    }
}

/// grpc-message 是百分号编码的 UTF-8
pub fn decode_message(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or_default();
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub code: u32,
    pub name: &'static str,
    pub message: String,
    /// 由客户端判定（截止时间、取消、协议错误），而不是服务器返回
    pub local: bool,
}

impl Status {
    pub fn local(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            name: status_name(code),
            message: message.into(),
            local: true,
        }
    }
}

/// 从响应头或 trailers 中读取 grpc-status
pub fn status_from(headers: &[(String, String)]) -> Option<Status> {
    let code = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("grpc-status"))
        .and_then(|(_, v)| v.trim().parse::<u32>().ok())?;
    let message = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("grpc-message"))
        .map(|(_, v)| decode_message(v))
        .unwrap_or_default();
    Some(Status {
        code,
        name: status_name(code),
        message,
        local: false,
    })
}

/// 对外展示的 metadata：去掉 HTTP/2 与 gRPC 内部的头
pub fn visible(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "grpc-status" | "grpc-message" | "content-type"))
        .cloned()
        .collect()
}

pub struct CallOptions<'a> {
    /// `/包名.服务/方法`
    pub path: String,
    pub target: &'a Target,
    pub metadata: &'a [MetadataEntry],
    pub deadline_ms: Option<u64>,
    pub body: Vec<u8>,
}

/// 发出调用并拿到响应头；等待期间可被任务取消
pub fn open(
    options: &CallOptions<'_>,
    ctx: Option<&dyn TaskContext>,
) -> PluginResult<(Stream, u64)> {
    let base = options.target.base()?;
    let deadline = options
        .deadline_ms
        .unwrap_or(DEFAULT_DEADLINE_MS)
        .clamp(1, MAX_DEADLINE_MS);
    let mut request = StreamRequest::new("POST", &format!("{base}{}", options.path));
    request.http2_prior_knowledge = base.starts_with("http://");
    request.timeout_ms = (deadline + GRACE_MS) as u32;
    request.headers = vec![
        ("content-type".into(), "application/grpc".into()),
        ("te".into(), "trailers".into()),
        ("grpc-timeout".into(), format!("{deadline}m")),
        ("grpc-accept-encoding".into(), "identity".into()),
        (
            "user-agent".into(),
            concat!("grpc-toolforge/", env!("CARGO_PKG_VERSION")).into(),
        ),
    ];
    request.headers.extend(metadata_headers(options.metadata)?);
    request.body = Some(frame(&options.body));
    let stream = Stream::open(&request, ctx).map_err(|e| {
        if e.code == "net.timeout" {
            PluginError::new("grpc.deadline")
        } else {
            e
        }
    })?;
    Ok((stream, deadline))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    /// 本批新到的消息（ProtoJSON）
    pub messages: Vec<String>,
    /// 至今的消息数与字节数
    pub count: usize,
    pub bytes: usize,
    pub done: bool,
    pub status: Option<Status>,
    pub trailers: Vec<(String, String)>,
    pub elapsed_ms: u64,
    /// 达到消息数或字节上限后提前结束
    pub truncated: bool,
}

/// 进行中的调用
pub struct Call {
    pub id: u64,
    stream: Option<Stream>,
    output: MessageDescriptor,
    decoder: Decoder,
    started: Instant,
    pub count: usize,
    pub bytes: usize,
    max_messages: usize,
    max_bytes: usize,
    pub status: Option<Status>,
    pub trailers: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub messages: Vec<String>,
    pub truncated: bool,
}

impl Call {
    pub fn new(
        id: u64,
        stream: Stream,
        output: MessageDescriptor,
        max_messages: usize,
        max_bytes: usize,
    ) -> Self {
        let head = stream.head().clone();
        let mut call = Self {
            id,
            stream: Some(stream),
            output,
            decoder: Decoder::default(),
            started: Instant::now(),
            count: 0,
            bytes: 0,
            max_messages: max_messages.clamp(1, DEFAULT_MAX_MESSAGES * 10),
            max_bytes: max_bytes.clamp(1024, DEFAULT_MAX_BYTES * 4),
            status: None,
            trailers: Vec::new(),
            headers: visible(&head.headers),
            messages: Vec::new(),
            truncated: false,
        };
        // 非 200 或不是 gRPC 响应：直接结束
        let content_type = head
            .headers
            .iter()
            .find(|(k, _)| k == "content-type")
            .map(|(_, v)| v.as_str())
            .unwrap_or_default();
        if let Some(status) = status_from(&head.headers) {
            // 只有 trailers 的响应（Trailers-Only）
            call.finish(status, Vec::new());
        } else if head.status != 200 {
            call.finish(
                Status::local(
                    http_status_code(head.status),
                    format!("HTTP {}", head.status),
                ),
                Vec::new(),
            );
        } else if !content_type.starts_with("application/grpc") {
            call.finish(
                Status::local(2, format!("unexpected content-type {content_type}")),
                Vec::new(),
            );
        }
        call
    }

    fn finish(&mut self, status: Status, trailers: Vec<(String, String)>) {
        self.status = Some(status);
        self.trailers = trailers;
        // drop 流：未读完时宿主中止连接（HTTP/2 发送 RST_STREAM）
        self.stream = None;
    }

    pub fn done(&self) -> bool {
        self.status.is_some()
    }

    /// 主动取消
    pub fn cancel(&mut self) {
        if !self.done() {
            self.finish(Status::local(1, "cancelled by user"), Vec::new());
        }
    }

    fn take_messages(&mut self, out: &mut Vec<String>) -> PluginResult<()> {
        while let Some(payload) = self.decoder.next()? {
            if self.count >= self.max_messages || self.bytes + payload.len() > self.max_bytes {
                self.truncated = true;
                self.finish(
                    Status::local(8, "client message or byte limit reached"),
                    Vec::new(),
                );
                return Ok(());
            }
            self.count += 1;
            self.bytes += payload.len();
            let json = message::decode_to_json(&self.output, &payload)?;
            self.messages.push(json.clone());
            out.push(json);
        }
        Ok(())
    }

    /// 最多等待 `budget` 读取新消息；收到消息后再等一小段以便合并成一批
    pub fn poll(&mut self, budget: Duration, ctx: Option<&dyn TaskContext>) -> Batch {
        let mut fresh = Vec::new();
        let until = Instant::now() + budget;
        while !self.done() && Instant::now() < until {
            if ctx.is_some_and(|c| c.is_cancelled()) {
                self.cancel();
                break;
            }
            let wait = until
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(200));
            let Some(stream) = self.stream.as_mut() else {
                break;
            };
            let event = stream.read(wait);
            match event {
                Ok(StreamEvent::Data(data)) => {
                    self.decoder.push(&data);
                    if let Err(err) = self.take_messages(&mut fresh) {
                        self.finish(Status::local(13, err.code), Vec::new());
                    }
                    if !fresh.is_empty() && fresh.len() >= 50 {
                        break;
                    }
                }
                Ok(StreamEvent::Pending) => {
                    if !fresh.is_empty() {
                        break;
                    }
                }
                Ok(StreamEvent::End(trailers)) => {
                    let status = if self.decoder.has_partial() {
                        Status::local(13, "incomplete message frame")
                    } else {
                        status_from(&trailers)
                            .unwrap_or_else(|| Status::local(2, "missing grpc-status"))
                    };
                    let shown = visible(&trailers);
                    self.finish(status, shown);
                }
                Err(err) => {
                    let status = match err.code.as_str() {
                        "net.timeout" => Status::local(4, "deadline exceeded"),
                        "task.cancelled" => Status::local(1, "cancelled"),
                        _ => Status::local(
                            14,
                            err.params
                                .get("detail")
                                .and_then(|d| d.as_str())
                                .unwrap_or(&err.code)
                                .to_owned(),
                        ),
                    };
                    self.finish(status, Vec::new());
                }
            }
        }
        Batch {
            messages: fresh,
            count: self.count,
            bytes: self.bytes,
            done: self.done(),
            status: self.status.clone(),
            trailers: self.trailers.clone(),
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            truncated: self.truncated,
        }
    }
}

/// HTTP 状态码到 gRPC 状态（gRPC over HTTP/2 规范的映射）
pub fn http_status_code(status: u16) -> u32 {
    match status {
        400 => 13,
        401 => 16,
        403 => 7,
        404 => 12,
        429 | 502 | 503 | 504 => 14,
        _ => 2,
    }
}

#[cfg(test)]
#[path = "grpc_test.rs"]
mod tests;
