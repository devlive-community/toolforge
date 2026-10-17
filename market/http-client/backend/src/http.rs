use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};
use tf_plugin_guest::net::{self, HttpRequest};

/// 响应体最大读取量
const MAX_BODY: u64 = 50 * 1024 * 1024;
/// 返回给前端展示的正文长度上限；完整内容留在后端，按需保存
const PREVIEW: usize = 1024 * 1024;
const MAX_REDIRECTS: u32 = 10;

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

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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
            Some((id, bytes)) if *id == args.id => net::write_file(&args.path, bytes),
            _ => Err(PluginError::new("http.response_expired")),
        }
    }
}

/// 没有协议时补全：本机与内网地址用 http，其余用 https
pub fn normalize_url(raw: &str) -> PluginResult<url::Url> {
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
    let url = url::Url::parse(&with_scheme)
        .map_err(|e| PluginError::new("http.invalid_url").with("detail", e.to_string()))?;
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
pub fn build_url(request: &Request) -> PluginResult<url::Url> {
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

/// HTTP token（RFC 9110）：方法名与请求头名称允许的字符
fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

fn method(raw: &str) -> PluginResult<String> {
    let upper = raw.trim().to_ascii_uppercase();
    if !is_token(&upper) {
        return Err(PluginError::new("http.invalid_method").with("method", raw));
    }
    Ok(upper)
}

/// 请求头：名称为 token，值只允许制表符与可见 ASCII
fn headers(request: &Request) -> PluginResult<Vec<(String, String)>> {
    active(&request.headers)
        .map(|pair| {
            let (name, value) = (pair.key.trim(), pair.value.trim());
            let valid_value = value
                .bytes()
                .all(|b| b == b'\t' || (0x20..0x7f).contains(&b));
            if !is_token(name) || !valid_value {
                return Err(PluginError::new("http.invalid_header").with("name", pair.key.as_str()));
            }
            Ok((name.to_owned(), value.to_owned()))
        })
        .collect()
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
                    url::Url::parse("http://x/").expect("static url"),
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

/// 宿主网络错误换成本插件的错误码；参数里只有底层原因，不含地址
fn network_error(err: PluginError) -> PluginError {
    let code = match err.code.as_str() {
        "net.timeout" => "http.timeout",
        "net.connect" => "http.connect_failed",
        "net.redirects" => "http.too_many_redirects",
        "net.too_large" => return PluginError::new("http.body_too_large").with("limit", "50 MB"),
        "net.failed" | "net.invalid_request" => "http.network",
        _ => return err,
    };
    let mut mapped = PluginError::new(code);
    mapped.params = err.params;
    mapped
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
    send_with_redacted_urls(request, bodies, ctx, false)
}

pub fn send_with_redacted_urls(
    request: Request,
    bodies: &Bodies,
    ctx: &dyn TaskContext,
    redact_urls: bool,
) -> PluginResult<Reply> {
    let method = method(&request.method)?;
    let url = build_url(&request)?;
    let mut header_list = headers(&request)?;
    let payload = body(&request)?;
    if let Some((_, content_type)) = &payload
        && !has_header(&request, "content-type")
    {
        header_list.push(("Content-Type".into(), (*content_type).to_owned()));
    }
    ctx.log(
        LogLevel::Info,
        "http.request",
        json!({ "method": method, "url": if redact_urls { "[redacted]" } else { url.as_str() } }),
    );
    // 请求由宿主发送：跨源重定向时宿主会去掉 Authorization 等凭据头
    let outgoing = HttpRequest {
        method,
        url: url.to_string(),
        headers: header_list,
        body: payload.map(|(bytes, _)| bytes),
        timeout_ms: request.timeout_ms.clamp(1000, 600_000) as u32,
        follow_redirects: request.follow_redirects,
        max_redirects: MAX_REDIRECTS,
        use_proxy: request.use_proxy,
        max_body: MAX_BODY,
    };
    let response = net::send(&outgoing, Some(ctx)).map_err(network_error)?;
    for hop in &response.redirects {
        ctx.log(
            LogLevel::Info,
            "http.redirect",
            json!({ "url": if redact_urls { "[redacted]" } else { hop } }),
        );
    }
    ctx.log(
        LogLevel::Info,
        "http.response",
        json!({ "status": response.status, "ms": response.elapsed_ms }),
    );
    ctx.log(
        LogLevel::Info,
        "http.body",
        json!({ "bytes": response.body.len(), "ms": response.elapsed_ms }),
    );
    let content_type = response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.clone());
    let bytes = response.body;
    let (kind, body, truncated) = render(&classify(content_type.as_deref(), &bytes), &bytes);
    let size = bytes.len() as u64;
    Ok(Reply {
        id: bodies.store(bytes),
        status: response.status,
        status_text: response.reason,
        version: response.version,
        url: response.url,
        elapsed_ms: response.elapsed_ms,
        size,
        headers: response.headers,
        content_type,
        kind,
        body,
        truncated,
        redirects: response.redirects,
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
    if method != "GET" || payload.is_some() {
        parts.push(format!("-X {method}"));
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
