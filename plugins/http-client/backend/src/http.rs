use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

/// 响应体最大读取量
const MAX_BODY: u64 = 50 * 1024 * 1024;
/// 返回给前端展示的正文长度上限；完整内容留在后端，按需保存
const PREVIEW: usize = 1024 * 1024;
const CHUNK: usize = 64 * 1024;
const MAX_REDIRECTS: usize = 10;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Pair {
    pub key: String,
    pub value: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    #[default]
    None,
    Json,
    Text,
    Form,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    #[serde(default)]
    pub kind: BodyKind,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub form: Vec<Pair>,
}

fn default_timeout() -> u64 {
    30_000
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub params: Vec<Pair>,
    #[serde(default)]
    pub headers: Vec<Pair>,
    #[serde(default)]
    pub body: Body,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "enabled")]
    pub follow_redirects: bool,
    /// 使用系统代理；本机地址始终直连
    #[serde(default = "enabled")]
    pub use_proxy: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Json,
    Text,
    Binary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    /// 完整响应体在后端缓存中的编号，用于保存
    pub id: u64,
    pub status: u16,
    pub status_text: String,
    pub version: String,
    pub url: String,
    pub elapsed_ms: u64,
    pub size: u64,
    pub headers: Vec<(String, String)>,
    pub content_type: Option<String>,
    pub kind: ContentKind,
    /// JSON 已格式化；二进制时为空
    pub body: String,
    pub truncated: bool,
    pub redirects: Vec<String>,
}

#[derive(Deserialize)]
pub struct SaveArgs {
    id: u64,
    path: String,
}

/// 最近一次响应的原始正文
#[derive(Default)]
pub struct Bodies {
    next: AtomicU64,
    latest: Mutex<Option<(u64, Vec<u8>)>>,
}

impl Bodies {
    fn store(&self, bytes: Vec<u8>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some((id, bytes));
        id
    }

    pub fn save(&self, args: SaveArgs) -> PluginResult<()> {
        let latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        match latest.as_ref() {
            Some((id, bytes)) if *id == args.id => std::fs::write(&args.path, bytes).map_err(|e| {
                PluginError::new("fs.io")
                    .with("path", args.path.as_str())
                    .with("detail", e.to_string())
            }),
            _ => Err(PluginError::new("http.response_expired")),
        }
    }
}

/// 没有协议时补全：本机与内网地址用 http，其余用 https
pub fn normalize_url(raw: &str) -> PluginResult<reqwest::Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PluginError::new("http.empty_url"));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        let host = trimmed.split(['/', ':', '?']).next().unwrap_or_default();
        let local = host == "localhost"
            || host.starts_with("127.")
            || host.starts_with("10.")
            || host.starts_with("192.168.")
            || host.ends_with(".local");
        format!("{}://{trimmed}", if local { "http" } else { "https" })
    };
    let url = reqwest::Url::parse(&with_scheme).map_err(|e| {
        PluginError::new("http.invalid_url")
            .with("url", trimmed)
            .with("detail", e.to_string())
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(PluginError::new("http.unsupported_scheme").with("scheme", url.scheme()));
    }
    Ok(url)
}

fn active(pairs: &[Pair]) -> impl Iterator<Item = &Pair> {
    pairs
        .iter()
        .filter(|p| p.enabled && !p.key.trim().is_empty())
}

/// 合并地址栏中的查询参数与参数表
pub fn build_url(request: &Request) -> PluginResult<reqwest::Url> {
    let mut url = normalize_url(&request.url)?;
    let params: Vec<_> = active(&request.params).collect();
    if !params.is_empty() {
        let mut query = url.query_pairs_mut();
        for p in params {
            query.append_pair(p.key.trim(), &p.value);
        }
    }
    Ok(url)
}

fn method(raw: &str) -> PluginResult<reqwest::Method> {
    let upper = raw.trim().to_ascii_uppercase();
    reqwest::Method::from_bytes(upper.as_bytes())
        .map_err(|_| PluginError::new("http.invalid_method").with("method", raw))
}

fn headers(request: &Request) -> PluginResult<HeaderMap> {
    let mut map = HeaderMap::new();
    for pair in active(&request.headers) {
        let name = HeaderName::from_bytes(pair.key.trim().as_bytes())
            .map_err(|_| PluginError::new("http.invalid_header").with("name", pair.key.as_str()))?;
        let value = HeaderValue::from_str(pair.value.trim())
            .map_err(|_| PluginError::new("http.invalid_header").with("name", pair.key.as_str()))?;
        map.append(name, value);
    }
    Ok(map)
}

fn has_header(request: &Request, name: &str) -> bool {
    active(&request.headers).any(|p| p.key.trim().eq_ignore_ascii_case(name))
}

/// 请求体与默认 Content-Type
pub fn body(request: &Request) -> PluginResult<Option<(Vec<u8>, &'static str)>> {
    Ok(match request.body.kind {
        BodyKind::None => None,
        BodyKind::Json => {
            if !request.body.text.trim().is_empty() {
                serde_json::from_str::<Value>(&request.body.text).map_err(|e| {
                    PluginError::new("http.invalid_json_body")
                        .with("line", e.line())
                        .with("column", e.column())
                })?;
            }
            Some((request.body.text.clone().into_bytes(), "application/json"))
        }
        BodyKind::Text => Some((
            request.body.text.clone().into_bytes(),
            "text/plain; charset=utf-8",
        )),
        BodyKind::Form => {
            let encoded = active(&request.body.form)
                .fold(
                    reqwest::Url::parse("http://x/").expect("static url"),
                    |mut url, p| {
                        url.query_pairs_mut().append_pair(p.key.trim(), &p.value);
                        url
                    },
                )
                .query()
                .unwrap_or_default()
                .to_owned();
            Some((encoded.into_bytes(), "application/x-www-form-urlencoded"))
        }
    })
}

fn is_loopback(url: &reqwest::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

fn client(
    request: &Request,
    url: &reqwest::Url,
    redirects: Arc<Mutex<Vec<String>>>,
) -> PluginResult<Client> {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
    let policy = if request.follow_redirects {
        Policy::custom(move |attempt| {
            if attempt.previous().len() > MAX_REDIRECTS {
                return attempt.error("too many redirects");
            }
            redirects
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(attempt.url().to_string());
            attempt.follow()
        })
    } else {
        Policy::none()
    };
    let mut builder = Client::builder();
    if !request.use_proxy || is_loopback(url) {
        builder = builder.no_proxy();
    }
    builder
        .user_agent(concat!("ToolForge/", env!("CARGO_PKG_VERSION")))
        .redirect(policy)
        .timeout(Duration::from_millis(
            request.timeout_ms.clamp(1000, 600_000),
        ))
        .build()
        .map_err(|e| PluginError::new("http.network").with("detail", e.to_string()))
}

fn network_error(err: reqwest::Error) -> PluginError {
    let code = if err.is_timeout() {
        "http.timeout"
    } else if err.is_connect() {
        "http.connect_failed"
    } else if err.is_redirect() {
        "http.too_many_redirects"
    } else {
        "http.network"
    };
    // reqwest 的错误信息包含底层原因（DNS、TLS 等）
    let mut detail = err.to_string();
    let mut source = std::error::Error::source(&err);
    while let Some(inner) = source {
        detail = format!("{detail}: {inner}");
        source = inner.source();
    }
    PluginError::new(code).with("detail", detail)
}

fn read_body(response: &mut Response, ctx: &dyn TaskContext) -> PluginResult<Vec<u8>> {
    let expected = response.content_length();
    if expected.is_some_and(|len| len > MAX_BODY) {
        return Err(PluginError::new("http.body_too_large").with("limit", "50 MB"));
    }
    let mut bytes = Vec::with_capacity(expected.unwrap_or(0).min(MAX_BODY) as usize);
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        let read = response
            .read(&mut buffer)
            .map_err(|e| PluginError::new("http.network").with("detail", e.to_string()))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() as u64 > MAX_BODY {
            return Err(PluginError::new("http.body_too_large").with("limit", "50 MB"));
        }
        if let Some(total) = expected {
            ctx.progress(bytes.len() as u64, total);
        }
    }
    Ok(bytes)
}

fn classify(content_type: Option<&str>, bytes: &[u8]) -> ContentKind {
    let ct = content_type.unwrap_or_default().to_ascii_lowercase();
    if ct.contains("json") {
        return ContentKind::Json;
    }
    let textual = ct.starts_with("text/")
        || [
            "xml",
            "javascript",
            "html",
            "yaml",
            "csv",
            "x-www-form-urlencoded",
        ]
        .iter()
        .any(|k| ct.contains(k));
    if textual || (ct.is_empty() && std::str::from_utf8(&bytes[..bytes.len().min(4096)]).is_ok()) {
        ContentKind::Text
    } else {
        ContentKind::Binary
    }
}

/// 生成展示用正文：JSON 格式化，文本按上限截断（保证 UTF-8 边界）
pub fn render(kind: &ContentKind, bytes: &[u8]) -> (ContentKind, String, bool) {
    match kind {
        ContentKind::Binary => (ContentKind::Binary, String::new(), false),
        ContentKind::Json => match serde_json::from_slice::<Value>(bytes) {
            Ok(value) if bytes.len() <= PREVIEW * 4 => {
                let pretty = serde_json::to_string_pretty(&value).unwrap_or_default();
                let (text, truncated) = truncate(&pretty);
                (ContentKind::Json, text, truncated)
            }
            _ => {
                let (text, truncated) = truncate(&String::from_utf8_lossy(bytes));
                (ContentKind::Text, text, truncated)
            }
        },
        ContentKind::Text => {
            let (text, truncated) = truncate(&String::from_utf8_lossy(bytes));
            (ContentKind::Text, text, truncated)
        }
    }
}

fn truncate(text: &str) -> (String, bool) {
    if text.len() <= PREVIEW {
        return (text.to_owned(), false);
    }
    let mut end = PREVIEW;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

pub fn send(request: Request, bodies: &Bodies, ctx: &dyn TaskContext) -> PluginResult<Reply> {
    let method = method(&request.method)?;
    let url = build_url(&request)?;
    let mut header_map = headers(&request)?;
    let payload = body(&request)?;
    if let Some((_, content_type)) = &payload
        && !has_header(&request, "content-type")
    {
        header_map.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    }

    let redirects = Arc::new(Mutex::new(Vec::new()));
    let client = client(&request, &url, redirects.clone())?;
    ctx.log(
        LogLevel::Info,
        "http.request",
        json!({ "method": method.as_str(), "url": url.as_str() }),
    );
    let start = Instant::now();
    let mut builder = client.request(method, url).headers(header_map);
    if let Some((bytes, _)) = payload {
        builder = builder.body(bytes);
    }
    let mut response = builder.send().map_err(network_error)?;
    let redirects = std::mem::take(&mut *redirects.lock().unwrap_or_else(|e| e.into_inner()));
    for hop in &redirects {
        ctx.log(LogLevel::Info, "http.redirect", json!({ "url": hop }));
    }
    let status = response.status();
    ctx.log(
        LogLevel::Info,
        "http.response",
        json!({ "status": status.as_u16(), "ms": start.elapsed().as_millis() as u64 }),
    );

    let final_url = response.url().to_string();
    let version = format!("{:?}", response.version());
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = read_body(&mut response, ctx)?;
    let elapsed_ms = start.elapsed().as_millis() as u64;
    ctx.log(
        LogLevel::Info,
        "http.body",
        json!({ "bytes": bytes.len(), "ms": elapsed_ms }),
    );

    let (kind, body, truncated) = render(&classify(content_type.as_deref(), &bytes), &bytes);
    let size = bytes.len() as u64;
    Ok(Reply {
        id: bodies.store(bytes),
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or_default().to_owned(),
        version,
        url: final_url,
        elapsed_ms,
        size,
        headers,
        content_type,
        kind,
        body,
        truncated,
        redirects,
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// 生成等价的 curl 命令
pub fn to_curl(request: Request) -> PluginResult<String> {
    let method = method(&request.method)?;
    let url = build_url(&request)?;
    let payload = body(&request)?;
    let mut parts = vec!["curl".to_owned()];
    if method != reqwest::Method::GET || payload.is_some() {
        parts.push(format!("-X {}", method.as_str()));
    }
    parts.push(shell_quote(url.as_str()));
    for pair in active(&request.headers) {
        parts.push(format!(
            "-H {}",
            shell_quote(&format!("{}: {}", pair.key.trim(), pair.value.trim()))
        ));
    }
    if let Some((bytes, content_type)) = payload {
        if !has_header(&request, "content-type") {
            parts.push(format!(
                "-H {}",
                shell_quote(&format!("Content-Type: {content_type}"))
            ));
        }
        parts.push(format!(
            "--data-raw {}",
            shell_quote(&String::from_utf8_lossy(&bytes))
        ));
    }
    if request.follow_redirects {
        parts.push("-L".into());
    }
    Ok(parts.join(" \\\n  "))
}

#[cfg(test)]
#[path = "http_test.rs"]
mod tests;
