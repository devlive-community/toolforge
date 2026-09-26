//! 建立 WebSocket 连接（ws / wss），在一个循环中收发消息，直到关闭或任务被取消。

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};
use tungstenite::client::IntoClientRequest;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::CloseFrame;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::{Message, WebSocket};

use crate::session::{Direction, Info, Outbound, Session, State};

/// 读取超时，决定发送队列与取消的响应速度
const POLL: Duration = Duration::from_millis(50);

fn default_timeout() -> u64 {
    10_000
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub session: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub protocols: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub sent: u64,
    pub received: u64,
    pub close_code: Option<u16>,
    pub close_reason: Option<String>,
}

pub enum Stream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    fn socket(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => &s.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

fn failed(code: &str, detail: impl ToString) -> PluginError {
    PluginError::new(code).with("detail", detail.to_string())
}

/// 依次尝试解析出的所有地址
fn dial(host: &str, port: u16, timeout: Duration) -> PluginResult<(TcpStream, String)> {
    let addresses: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|_| PluginError::new("ws.resolve_failed").with("host", host))?
        .collect();
    let mut last = None;
    for address in &addresses {
        match TcpStream::connect_timeout(address, timeout) {
            Ok(stream) => return Ok((stream, address.to_string())),
            Err(err) => last = Some(err),
        }
    }
    Err(match last {
        Some(err) if err.kind() == ErrorKind::TimedOut => PluginError::new("ws.timeout"),
        Some(err) if err.kind() == ErrorKind::ConnectionRefused => PluginError::new("ws.refused"),
        Some(err) => failed("ws.connect_failed", err),
        None => PluginError::new("ws.resolve_failed").with("host", host),
    })
}

fn tls(host: &str, socket: TcpStream) -> PluginResult<Stream> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = rustls_platform_verifier::Verifier::new(provider.clone())
        .map_err(|e| failed("ws.tls_failed", e))?;
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| failed("ws.tls_failed", e))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = rustls::pki_types::ServerName::try_from(host.to_owned())
        .map_err(|_| PluginError::new("ws.invalid_url"))?;
    let connection = rustls::ClientConnection::new(Arc::new(config), name)
        .map_err(|e| failed("ws.tls_failed", e))?;
    Ok(Stream::Tls(Box::new(rustls::StreamOwned::new(
        connection, socket,
    ))))
}

fn build_request(args: &Args) -> PluginResult<tungstenite::handshake::client::Request> {
    let parsed =
        url::Url::parse(args.url.trim()).map_err(|_| PluginError::new("ws.invalid_url"))?;
    if !matches!(parsed.scheme(), "ws" | "wss") {
        return Err(PluginError::new("ws.invalid_scheme").with("scheme", parsed.scheme()));
    }
    let mut request = parsed
        .as_str()
        .into_client_request()
        .map_err(|_| PluginError::new("ws.invalid_url"))?;
    let headers = request.headers_mut();
    for (name, value) in &args.headers {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let key = tungstenite::http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| PluginError::new("ws.invalid_header").with("name", name))?;
        let value = tungstenite::http::HeaderValue::from_str(value.trim())
            .map_err(|_| PluginError::new("ws.invalid_header").with("name", name))?;
        headers.append(key, value);
    }
    let protocols: Vec<&str> = args
        .protocols
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if !protocols.is_empty() {
        let value =
            tungstenite::http::HeaderValue::from_str(&protocols.join(", ")).map_err(|_| {
                PluginError::new("ws.invalid_header").with("name", "Sec-WebSocket-Protocol")
            })?;
        headers.insert("Sec-WebSocket-Protocol", value);
    }
    Ok(request)
}

fn is_timeout(err: &tungstenite::Error) -> bool {
    matches!(err, tungstenite::Error::Io(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}

fn handshake(args: &Args, session: &Session) -> PluginResult<WebSocket<Stream>> {
    let request = build_request(args)?;
    let uri = request.uri().clone();
    let host = uri
        .host()
        .ok_or_else(|| PluginError::new("ws.invalid_url"))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let secure = uri.scheme_str() == Some("wss");
    let port = uri.port_u16().unwrap_or(if secure { 443 } else { 80 });
    let timeout = Duration::from_millis(args.timeout_ms.clamp(500, 60_000));
    let started = Instant::now();
    let (socket, address) = dial(&host, port, timeout)?;
    let _ = socket.set_read_timeout(Some(timeout));
    let _ = socket.set_write_timeout(Some(timeout));
    let _ = socket.set_nodelay(true);
    let stream = if secure {
        tls(&host, socket)?
    } else {
        Stream::Plain(socket)
    };
    let (socket, response) = tungstenite::client(request, stream).map_err(|err| match err {
        HandshakeError::Failure(tungstenite::Error::Http(response)) => {
            let body = response.body().as_ref().map(|b| {
                String::from_utf8_lossy(b)
                    .chars()
                    .take(500)
                    .collect::<String>()
            });
            PluginError::new("ws.rejected")
                .with("status", response.status().as_u16())
                .with("body", body.unwrap_or_default())
        }
        HandshakeError::Failure(tungstenite::Error::Io(e))
            if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
        {
            PluginError::new("ws.timeout")
        }
        HandshakeError::Failure(tungstenite::Error::Io(e))
            if e.to_string().contains("certificate")
                || e.to_string().contains("InvalidCertificate") =>
        {
            failed("ws.tls_failed", e)
        }
        HandshakeError::Failure(other) => failed("ws.handshake_failed", other),
        HandshakeError::Interrupted(_) => PluginError::new("ws.timeout"),
    })?;
    session.set_info(Info {
        url: args.url.trim().to_owned(),
        address: Some(address),
        status: Some(response.status().as_u16()),
        protocol: response
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        headers: response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect(),
        handshake_ms: Some(started.elapsed().as_millis() as u64),
    });
    socket
        .get_ref()
        .socket()
        .set_read_timeout(Some(POLL))
        .map_err(|e| failed("ws.connect_failed", e))?;
    Ok(socket)
}

pub fn run(args: &Args, session: &Session, ctx: &dyn TaskContext) -> PluginResult<Summary> {
    session.set_state(State::Connecting);
    session.set_info(Info {
        url: args.url.trim().to_owned(),
        ..Info::default()
    });
    ctx.stage("ws.connect");
    let mut socket = match handshake(args, session) {
        Ok(socket) => socket,
        Err(err) => {
            session.event(
                "error",
                Some(err.code.clone()),
                err.params
                    .get("detail")
                    .and_then(|d| d.as_str())
                    .map(str::to_owned),
            );
            session.set_state(State::Closed);
            return Err(err);
        }
    };
    session.set_state(State::Open);
    session.event("open", None, None);
    ctx.stage("ws.open");
    ctx.log(
        LogLevel::Info,
        "ws.opened",
        json!({ "url": args.url.trim() }),
    );

    let mut summary = Summary::default();
    let mut closing = false;
    let result = 'connection: loop {
        if ctx.is_cancelled() && !closing {
            session.enqueue(Outbound::Close(1000, String::new()));
        }
        for outbound in session.take_outbound() {
            let (message, record): (Message, Box<dyn FnOnce()>) = match outbound {
                Outbound::Text(text) => {
                    let copy = text.clone();
                    (
                        Message::text(text),
                        Box::new(move || session.text(Direction::Out, &copy)),
                    )
                }
                Outbound::Binary(bytes) => {
                    let copy = bytes.clone();
                    (
                        Message::binary(bytes),
                        Box::new(move || session.binary(Direction::Out, "binary", &copy)),
                    )
                }
                Outbound::Ping(bytes) => {
                    let copy = bytes.clone();
                    (
                        Message::Ping(bytes.into()),
                        Box::new(move || session.binary(Direction::Out, "ping", &copy)),
                    )
                }
                Outbound::Close(code, reason) => {
                    if closing {
                        continue;
                    }
                    closing = true;
                    session.set_state(State::Closing);
                    let reason_copy = reason.clone();
                    let frame = CloseFrame {
                        code: CloseCode::from(code),
                        reason: reason.into(),
                    };
                    (
                        Message::Close(Some(frame)),
                        Box::new(move || {
                            session.event(
                                "close",
                                Some(code.to_string()),
                                Some(reason_copy).filter(|r| !r.is_empty()),
                            )
                        }),
                    )
                }
            };
            match socket.send(message) {
                Ok(()) => {
                    summary.sent += 1;
                    record();
                }
                Err(err) if is_timeout(&err) => {}
                Err(err) => break 'connection Err(err),
            }
        }
        match socket.read() {
            Ok(message) => {
                summary.received += 1;
                match message {
                    Message::Text(text) => session.text(Direction::In, text.as_str()),
                    Message::Binary(bytes) => session.binary(Direction::In, "binary", &bytes),
                    Message::Ping(bytes) => session.binary(Direction::In, "ping", &bytes),
                    Message::Pong(bytes) => session.binary(Direction::In, "pong", &bytes),
                    Message::Close(frame) => {
                        let (code, reason) = frame.map_or((None, None), |f| {
                            (Some(u16::from(f.code)), Some(f.reason.to_string()))
                        });
                        summary.close_code = code;
                        summary.close_reason = reason.clone().filter(|r| !r.is_empty());
                        session.event(
                            "close",
                            code.map(|c| c.to_string()),
                            summary.close_reason.clone(),
                        );
                        session.set_state(State::Closing);
                        closing = true;
                    }
                    Message::Frame(_) => {}
                }
            }
            Err(err) if is_timeout(&err) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                break 'connection Ok(());
            }
            Err(err) => break 'connection Err(err),
        }
    };
    session.set_state(State::Closed);
    match result {
        Ok(()) => {
            ctx.log(LogLevel::Info, "ws.closed", json!({ "sent": summary.sent, "received": summary.received, "code": summary.close_code }));
            Ok(summary)
        }
        Err(err) => {
            // 对方直接断开 TCP 也视为连接结束
            let code = match &err {
                tungstenite::Error::Protocol(
                    tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                ) => "ws.reset",
                _ => "ws.connection_lost",
            };
            session.event("error", Some(code.to_owned()), Some(err.to_string()));
            ctx.log(LogLevel::Warn, code, json!({ "detail": err.to_string() }));
            Ok(summary)
        }
    }
}

#[cfg(test)]
#[path = "connect_test.rs"]
mod tests;
