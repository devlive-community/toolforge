//! 流式 HTTP 请求：逐块读取响应体并在结束时拿到 trailers，供 gRPC、SSE 等使用。
//!
//! 直接基于 hyper：https 用 rustls 与系统证书校验，ALPN 协商 HTTP/2 或 HTTP/1.1；
//! http 地址可选 HTTP/2 先验知识（h2c，gRPC 明文需要），否则用 HTTP/1.1。不使用代理。
//! 整个流有截止时间；`HttpStream` 被 drop 时中止读取与连接（HTTP/2 下发送 RST_STREAM）。

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::{http1, http2};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tf_plugin_api::net::{StreamEvent, StreamHead, StreamRequest};
use tf_plugin_api::{PluginError, PluginResult};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::ensure_crypto_provider;

/// 流的截止时间上限
const MAX_STREAM: Duration = Duration::from_secs(3600);
/// 读取端积压的响应块上限；读取慢时对端被背压
const BUFFER: usize = 64;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("plugin-net")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn failed(detail: impl ToString) -> PluginError {
    PluginError::new("net.failed").with("detail", detail.to_string())
}

fn invalid(detail: impl ToString) -> PluginError {
    PluginError::new("net.invalid_request").with("detail", detail.to_string())
}

/// hyper 错误：连接被拒、证书错误等都带上底层原因
fn hyper_error(err: &(dyn std::error::Error + 'static)) -> String {
    let mut detail = err.to_string();
    let mut source = err.source();
    while let Some(inner) = source {
        detail = format!("{detail}: {inner}");
        source = inner.source();
    }
    detail
}

enum Frame {
    Data(Vec<u8>),
    End(Vec<(String, String)>),
}

type Message = PluginResult<Frame>;

/// 打开的流；drop 时中止
pub struct HttpStream {
    rx: mpsc::Receiver<Message>,
    tasks: Vec<JoinHandle<()>>,
    finished: bool,
}

impl Drop for HttpStream {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn headers_of(map: &http::HeaderMap) -> Vec<(String, String)> {
    map.iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect()
}

fn tls_config(alpn: Vec<Vec<u8>>) -> PluginResult<Arc<rustls::ClientConfig>> {
    use rustls_platform_verifier::ConfigVerifierExt;
    let mut config = rustls::ClientConfig::with_platform_verifier()
        .map_err(|e| PluginError::new("net.tls").with("detail", e.to_string()))?;
    config.alpn_protocols = alpn;
    Ok(Arc::new(config))
}

enum Sender {
    H1(http1::SendRequest<Full<Bytes>>),
    H2(http2::SendRequest<Full<Bytes>>),
}

/// 在连接上完成握手，连接驱动任务放进 tasks
async fn handshake<S>(io: S, h2: bool, tasks: &mut Vec<JoinHandle<()>>) -> PluginResult<Sender>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let io = TokioIo::new(io);
    if h2 {
        let (sender, conn) = http2::handshake(TokioExecutor::new(), io)
            .await
            .map_err(|e| PluginError::new("net.connect").with("detail", hyper_error(&e)))?;
        tasks.push(tokio::spawn(async move {
            let _ = conn.await;
        }));
        Ok(Sender::H2(sender))
    } else {
        let (sender, conn) = http1::handshake(io)
            .await
            .map_err(|e| PluginError::new("net.connect").with("detail", hyper_error(&e)))?;
        tasks.push(tokio::spawn(async move {
            let _ = conn.await;
        }));
        Ok(Sender::H1(sender))
    }
}

async fn connect(
    request: &StreamRequest,
    url: &url::Url,
    tasks: &mut Vec<JoinHandle<()>>,
) -> PluginResult<(http::Response<Incoming>, bool)> {
    let host = url.host().ok_or_else(|| invalid("missing host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| invalid("missing port"))?;
    let tcp = match &host {
        url::Host::Domain(d) => TcpStream::connect((*d, port)).await,
        url::Host::Ipv4(ip) => TcpStream::connect((*ip, port)).await,
        url::Host::Ipv6(ip) => TcpStream::connect((*ip, port)).await,
    }
    .map_err(|e| PluginError::new("net.connect").with("detail", e.to_string()))?;
    let _ = tcp.set_nodelay(true);

    let (sender, h2) = if url.scheme() == "https" {
        let alpn = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(tls_config(alpn)?);
        let name = rustls::pki_types::ServerName::try_from(host.to_string())
            .map_err(|e| invalid(e.to_string()))?;
        let tls = connector
            .connect(name, tcp)
            .await
            .map_err(|e| PluginError::new("net.tls").with("detail", e.to_string()))?;
        let h2 = tls.get_ref().1.alpn_protocol() == Some(b"h2");
        (handshake(tls, h2, tasks).await?, h2)
    } else {
        let h2 = request.http2_prior_knowledge;
        (handshake(tcp, h2, tasks).await?, h2)
    };

    let method = http::Method::from_bytes(request.method.trim().as_bytes()).map_err(invalid)?;
    // HTTP/2 用完整地址生成 :scheme / :authority；HTTP/1.1 用路径加 Host 头
    let target = if h2 {
        url.as_str().to_owned()
    } else {
        let mut path = url.path().to_owned();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        path
    };
    let mut builder = http::Request::builder().method(method).uri(target);
    if !h2 {
        let authority = match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
            None => url.host_str().unwrap_or_default().to_owned(),
        };
        builder = builder.header(http::header::HOST, authority);
    }
    for (name, value) in &request.headers {
        let name = http::HeaderName::from_bytes(name.trim().as_bytes()).map_err(invalid)?;
        let value = http::HeaderValue::from_str(value.trim()).map_err(invalid)?;
        builder = builder.header(name, value);
    }
    let body = Full::new(Bytes::from(request.body.clone().unwrap_or_default()));
    let outgoing = builder.body(body).map_err(invalid)?;
    let response = match sender {
        Sender::H1(mut s) => s.send_request(outgoing).await,
        Sender::H2(mut s) => s.send_request(outgoing).await,
    }
    .map_err(|e| failed(hyper_error(&e)))?;
    Ok((response, h2))
}

/// 读取响应体直到结束或截止，逐块送进通道
async fn pump(mut body: Incoming, deadline: Instant, tx: mpsc::Sender<Message>) {
    let mut trailers = Vec::new();
    loop {
        let frame = match tokio::time::timeout_at(deadline, body.frame()).await {
            Err(_) => {
                let _ = tx.send(Err(PluginError::new("net.timeout"))).await;
                return;
            }
            Ok(None) => break,
            Ok(Some(Err(e))) => {
                let _ = tx.send(Err(failed(hyper_error(&e)))).await;
                return;
            }
            Ok(Some(Ok(frame))) => frame,
        };
        match frame.into_data() {
            Ok(data) => {
                if !data.is_empty() && tx.send(Ok(Frame::Data(data.to_vec()))).await.is_err() {
                    return;
                }
            }
            Err(frame) => {
                if let Ok(map) = frame.into_trailers() {
                    trailers.extend(headers_of(&map));
                }
            }
        }
    }
    let _ = tx.send(Ok(Frame::End(trailers))).await;
}

impl HttpStream {
    /// 连接并发送请求，返回响应头；之后用 `read` 逐块读取响应体
    pub fn open(request: &StreamRequest) -> PluginResult<(Self, StreamHead)> {
        ensure_crypto_provider();
        let url = url::Url::parse(&request.url).map_err(invalid)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(PluginError::new("net.unsupported_scheme").with("scheme", url.scheme()));
        }
        let limit = Duration::from_millis(u64::from(request.timeout_ms))
            .clamp(Duration::from_secs(1), MAX_STREAM);
        let rt = runtime();
        let mut tasks = Vec::new();
        let opened = rt.block_on(async {
            let deadline = Instant::now() + limit;
            let response = tokio::time::timeout_at(deadline, connect(request, &url, &mut tasks))
                .await
                .map_err(|_| PluginError::new("net.timeout"))??;
            Ok::<_, PluginError>((response, deadline))
        });
        let ((response, h2), deadline) = match opened {
            Ok(value) => value,
            Err(err) => {
                for task in &tasks {
                    task.abort();
                }
                return Err(err);
            }
        };
        let (parts, body) = response.into_parts();
        let head = StreamHead {
            status: parts.status.as_u16(),
            reason: parts
                .status
                .canonical_reason()
                .unwrap_or_default()
                .to_owned(),
            version: if h2 {
                "HTTP/2".into()
            } else {
                "HTTP/1.1".into()
            },
            headers: headers_of(&parts.headers),
        };
        let (tx, rx) = mpsc::channel(BUFFER);
        tasks.push(rt.spawn(pump(body, deadline, tx)));
        Ok((
            Self {
                rx,
                tasks,
                finished: false,
            },
            head,
        ))
    }

    /// 最多等待 `wait`：返回一块响应体、`Pending`（暂无数据）或 `End`（带 trailers）
    pub fn read(&mut self, wait: Duration) -> PluginResult<StreamEvent> {
        if self.finished {
            return Err(PluginError::new("net.stream_closed"));
        }
        // 计时器要在运行时上下文中创建
        let rx = &mut self.rx;
        let received = runtime().block_on(async { tokio::time::timeout(wait, rx.recv()).await });
        match received {
            Err(_) => Ok(StreamEvent::Pending),
            Ok(None) => {
                self.finished = true;
                Err(PluginError::new("net.stream_closed"))
            }
            Ok(Some(Ok(Frame::Data(data)))) => Ok(StreamEvent::Data(data)),
            Ok(Some(Ok(Frame::End(trailers)))) => {
                self.finished = true;
                Ok(StreamEvent::End(trailers))
            }
            Ok(Some(Err(err))) => {
                self.finished = true;
                Err(err)
            }
        }
    }
}

#[cfg(test)]
#[path = "stream_test.rs"]
mod tests;
