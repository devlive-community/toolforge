//! 插件网络能力的原生实现：宿主为在线插件执行 HTTP 请求、监听本机回调、写入用户选择的文件；
//! `tf-plugin-guest` 在本机构建（单元测试）时也直接使用这里的实现，行为与宿主一致。
//!
//! - HTTP 请求：reqwest（rustls），记录重定向；跨源重定向时 reqwest 会去掉 Authorization 等凭据头。
//!   本机地址始终直连，其余可选系统代理；响应体按上限读取，可取消。
//! - 回调监听：只绑定 127.0.0.1，只接受第一个匹配路径的 GET 请求，随后关闭；超时或关闭即释放端口。

use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest::redirect::Policy;
use tf_plugin_api::net::{HttpRequest, HttpResponse};
use tf_plugin_api::{PluginError, PluginResult, cancelled};

/// 宿主允许的最大响应体
pub const MAX_BODY: u64 = 100 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 20;
const CHUNK: usize = 64 * 1024;
/// 回调等待时长上限
pub const MAX_CALLBACK_WAIT: Duration = Duration::from_secs(1800);
const POLL: Duration = Duration::from_millis(100);

/// reqwest 使用 rustls 时需要先安装加密实现
pub fn ensure_crypto_provider() {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
}

fn is_loopback(url: &reqwest::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

fn invalid(detail: impl ToString) -> PluginError {
    PluginError::new("net.invalid_request").with("detail", detail.to_string())
}

fn network_error(err: reqwest::Error) -> PluginError {
    let err = err.without_url();
    let code = if err.is_timeout() {
        "net.timeout"
    } else if err.is_connect() {
        "net.connect"
    } else if err.is_redirect() {
        "net.redirects"
    } else {
        "net.failed"
    };
    // 错误信息包含底层原因（DNS、TLS 等），不含地址
    let mut detail = err.to_string();
    let mut source = std::error::Error::source(&err);
    while let Some(inner) = source {
        detail = format!("{detail}: {inner}");
        source = inner.source();
    }
    PluginError::new(code).with("detail", detail)
}

fn too_large(limit: u64) -> PluginError {
    PluginError::new("net.too_large").with("limit", limit)
}

/// 发送请求并读取完整响应；`cancelled` 在读取响应体时检查，`progress` 报告已读字节
pub fn send(
    request: &HttpRequest,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(u64, u64),
) -> PluginResult<HttpResponse> {
    ensure_crypto_provider();
    let url = reqwest::Url::parse(&request.url).map_err(invalid)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(PluginError::new("net.unsupported_scheme").with("scheme", url.scheme()));
    }
    let method = reqwest::Method::from_bytes(request.method.trim().as_bytes()).map_err(invalid)?;
    let mut headers = HeaderMap::new();
    for (name, value) in &request.headers {
        let name = HeaderName::from_bytes(name.trim().as_bytes()).map_err(invalid)?;
        let value = HeaderValue::from_str(value.trim()).map_err(invalid)?;
        headers.append(name, value);
    }
    if !headers.contains_key(USER_AGENT) {
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("ToolForge/", env!("CARGO_PKG_VERSION"))),
        );
    }
    let max_body = request.max_body.min(MAX_BODY);
    let redirects = Arc::new(Mutex::new(Vec::new()));
    let policy = if request.follow_redirects {
        let max = request.max_redirects.min(MAX_REDIRECTS) as usize;
        let seen = redirects.clone();
        Policy::custom(move |attempt| {
            if attempt.previous().len() > max {
                return attempt.error("too many redirects");
            }
            seen.lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(attempt.url().to_string());
            attempt.follow()
        })
    } else {
        Policy::none()
    };
    let mut builder = reqwest::blocking::Client::builder()
        .redirect(policy)
        .timeout(Duration::from_millis(u64::from(
            request.timeout_ms.clamp(1000, 600_000),
        )));
    if !request.use_proxy || is_loopback(&url) {
        builder = builder.no_proxy();
    }
    let client = builder.build().map_err(network_error)?;
    let start = Instant::now();
    let mut call = client.request(method, url).headers(headers);
    if let Some(body) = &request.body {
        call = call.body(body.clone());
    }
    let mut response = call.send().map_err(network_error)?;
    let expected = response.content_length();
    if expected.is_some_and(|len| len > max_body) {
        return Err(too_large(max_body));
    }
    let mut body = Vec::with_capacity(expected.unwrap_or(0).min(max_body) as usize);
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if cancel() {
            return Err(cancelled());
        }
        let read = response
            .read(&mut buffer)
            .map_err(|e| PluginError::new("net.failed").with("detail", e.to_string()))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read]);
        if body.len() as u64 > max_body {
            return Err(too_large(max_body));
        }
        if let Some(total) = expected {
            progress(body.len() as u64, total);
        }
    }
    let status = response.status();
    let redirects = std::mem::take(&mut *redirects.lock().unwrap_or_else(|e| e.into_inner()));
    Ok(HttpResponse {
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or_default().to_owned(),
        version: format!("{:?}", response.version()),
        url: response.url().to_string(),
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
        body,
        redirects,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

const CALLBACK_PAGE: &str = "<!doctype html><meta charset=utf-8><title>ToolForge</title>\
<body style=\"font-family:system-ui;padding:3rem;text-align:center\">\
<h2>已收到回调 / Callback received</h2>\
<p>可以关闭此页面并回到 ToolForge 查看结果。<br>You can close this tab and return to ToolForge.</p>";

#[derive(Default)]
struct Shared {
    /// 收到的第一个回调的查询字符串
    query: Option<String>,
    timed_out: bool,
}

/// 一次性的本机回调监听
pub struct CallbackServer {
    port: u16,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 128
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-._~".contains(&b))
}

impl CallbackServer {
    /// 在 127.0.0.1 上监听；端口为 0 时由系统分配
    pub fn listen(port: u16, path: &str, timeout: Duration) -> PluginResult<Self> {
        if !valid_path(path) {
            return Err(PluginError::new("net.invalid_path"));
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                PluginError::new("net.port_in_use").with("port", port)
            } else {
                PluginError::new("net.listen_failed").with("detail", e.to_string())
            }
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| PluginError::new("net.listen_failed").with("detail", e.to_string()))?
            .port();
        let server = tiny_http::Server::from_listener(listener, None)
            .map_err(|e| PluginError::new("net.listen_failed").with("detail", e.to_string()))?;
        let shared: Arc<Mutex<Shared>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + timeout.clamp(Duration::from_secs(1), MAX_CALLBACK_WAIT);
        let path = path.to_owned();
        let thread = {
            let (shared, stop) = (shared.clone(), stop.clone());
            std::thread::Builder::new()
                .name("plugin-callback".into())
                .spawn(move || accept(server, &path, deadline, &shared, &stop))
                .map_err(|e| PluginError::new("net.listen_failed").with("detail", e.to_string()))?
        };
        Ok(Self {
            port,
            shared,
            stop,
            thread: Some(thread),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// 收到回调时返回其查询字符串；还在等待时为 None；超时返回 `net.callback_timeout`
    pub fn poll(&self) -> PluginResult<Option<String>> {
        let shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(query) = &shared.query {
            return Ok(Some(query.clone()));
        }
        if shared.timed_out {
            return Err(PluginError::new("net.callback_timeout"));
        }
        Ok(None)
    }
}

impl Drop for CallbackServer {
    /// 停止监听并等待线程退出，端口随之释放
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn accept(
    server: tiny_http::Server,
    path: &str,
    deadline: Instant,
    shared: &Mutex<Shared>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::SeqCst) {
        if Instant::now() >= deadline {
            shared.lock().unwrap_or_else(|e| e.into_inner()).timed_out = true;
            return;
        }
        let Ok(Some(request)) = server.recv_timeout(POLL) else {
            continue;
        };
        let target = request.url().to_owned();
        let (request_path, query) = target.split_once('?').unwrap_or((&target, ""));
        if *request.method() != tiny_http::Method::Get || request_path != path {
            let _ = request.respond(tiny_http::Response::empty(404));
            continue;
        }
        let header = tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8")
            .expect("static header");
        let _ =
            request.respond(tiny_http::Response::from_string(CALLBACK_PAGE).with_header(header));
        shared.lock().unwrap_or_else(|e| e.into_inner()).query = Some(query.to_owned());
        return;
    }
}

/// 写入用户选择的文件（覆盖）
pub fn write_file(path: &str, data: &[u8]) -> PluginResult<()> {
    std::fs::write(path, data).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", path)
            .with("detail", e.to_string())
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
