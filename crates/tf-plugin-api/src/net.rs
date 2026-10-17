//! 宿主提供给在线插件的网络能力所用的数据类型（需在 manifest 中声明 `network`）。
//!
//! 请求由宿主发送：插件只描述请求、拿到完整响应，不直接接触套接字。
//! 原生实现在 `tf-net`，WASM 插件经 `tf-plugin-guest::net` 调用宿主。

/// 需要声明的权限
pub const NETWORK_PERMISSION: &str = "network";

#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub method: String,
    /// 完整的 http / https 地址
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout_ms: u32,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    /// 使用系统代理；本机地址始终直连
    pub use_proxy: bool,
    /// 响应体读取上限（字节）
    pub max_body: u64,
}

impl HttpRequest {
    pub fn new(method: &str, url: &str) -> Self {
        Self {
            method: method.to_owned(),
            url: url.to_owned(),
            headers: Vec::new(),
            body: None,
            timeout_ms: 30_000,
            follow_redirects: true,
            max_redirects: 10,
            use_proxy: true,
            max_body: 50 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    /// 状态码的标准说明，如 `OK`
    pub reason: String,
    /// 如 `HTTP/1.1`
    pub version: String,
    /// 跟随重定向后的最终地址
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 依次经过的重定向地址
    pub redirects: Vec<String>,
    pub elapsed_ms: u64,
}

/// 流式请求：逐块读取响应体并拿到 trailers（gRPC、SSE 等）
#[derive(Debug, Clone, PartialEq)]
pub struct StreamRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// 整个流的截止时间（毫秒），超时后流以 `net.timeout` 结束
    pub timeout_ms: u32,
    /// http:// 地址直接使用 HTTP/2（h2c，gRPC 明文需要）；https 由 ALPN 协商
    pub http2_prior_knowledge: bool,
}

impl StreamRequest {
    pub fn new(method: &str, url: &str) -> Self {
        Self {
            method: method.to_owned(),
            url: url.to_owned(),
            headers: Vec::new(),
            body: None,
            timeout_ms: 60_000,
            http2_prior_knowledge: false,
        }
    }
}

/// 响应状态与响应头
#[derive(Debug, Clone, PartialEq)]
pub struct StreamHead {
    pub status: u16,
    pub reason: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// 一块响应体
    Data(Vec<u8>),
    /// 等待期间没有新数据
    Pending,
    /// 响应结束，附带 trailers（没有时为空）
    End(Vec<(String, String)>),
}
