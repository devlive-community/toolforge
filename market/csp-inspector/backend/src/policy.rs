//! CSP 策略解析与来源表达式匹配。
//!
//! 输入约定：一行一条策略（一个响应头的值）；行首写 `report-only:` 前缀
//! 表示 CSP-Report-Only。重复指令按规范只保留第一条并给出提示；未知指令
//! 保留并提示，不判整条策略失效。

use tf_plugin_api::PluginError;
use url::Url;

use crate::Diag;

pub const MAX_POLICIES: usize = 20;
pub const MAX_RESOURCES: usize = 500;
pub const MAX_TEXT: usize = 256 * 1024;

/// 常见指令；不在此列的指令只提示，不影响解析
pub const KNOWN_DIRECTIVES: &[&str] = &[
    "default-src",
    "script-src",
    "script-src-elem",
    "script-src-attr",
    "style-src",
    "style-src-elem",
    "style-src-attr",
    "img-src",
    "media-src",
    "font-src",
    "connect-src",
    "frame-src",
    "child-src",
    "worker-src",
    "object-src",
    "manifest-src",
    "base-uri",
    "form-action",
    "frame-ancestors",
    "report-uri",
    "report-to",
    "sandbox",
    "require-trusted-types-for",
    "trusted-types",
    "upgrade-insecure-requests",
    "prefetch-src",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Directive {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug)]
pub struct Policy {
    pub report_only: bool,
    pub raw: String,
    pub directives: Vec<Directive>,
    /// 策略里出现 upgrade-insecure-requests：匹配前把 http 资源升级为 https
    pub upgrade: bool,
    /// script-src 含 strict-dynamic：依赖执行链，静态只能给出边界结论
    pub strict_dynamic_script: bool,
    pub diagnostics: Vec<Diag>,
}

impl Policy {
    pub fn directive(&self, name: &str) -> Option<&Directive> {
        self.directives.iter().find(|d| d.name == name)
    }
}

/// 解析整段输入：每个非空行一条策略
pub fn parse_policies(text: &str) -> Result<Vec<Policy>, PluginError> {
    let mut policies = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if policies.len() >= MAX_POLICIES {
            return Err(PluginError::new("csp.too_many_policies").with("max", MAX_POLICIES));
        }
        let (report_only, rest) = match trimmed.split_once(':') {
            Some((head, tail))
                if head.eq_ignore_ascii_case("report-only") && !tail.starts_with("//") =>
            {
                (true, tail.trim())
            }
            _ => (false, trimmed),
        };
        policies.push(parse_policy(report_only, rest));
    }
    if policies.is_empty() {
        return Err(PluginError::new("csp.no_policies"));
    }
    Ok(policies)
}

fn parse_policy(report_only: bool, raw: &str) -> Policy {
    let mut diagnostics = Vec::new();
    let mut directives: Vec<Directive> = Vec::new();
    let mut upgrade = false;
    let mut strict_dynamic_script = false;
    for part in raw.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut tokens = part.split_whitespace();
        let Some(name_token) = tokens.next() else {
            continue;
        };
        let name = name_token.to_ascii_lowercase();
        let values: Vec<String> = tokens.map(str::to_owned).collect();
        if !KNOWN_DIRECTIVES.contains(&name.as_str()) {
            diagnostics.push(Diag::new("csp.unknown_directive").param("name", name.clone()));
        }
        if directives.iter().any(|d| d.name == name) {
            // 规范：同名指令只保留第一条
            diagnostics.push(Diag::new("csp.duplicate_directive").param("name", name.clone()));
            continue;
        }
        if name == "upgrade-insecure-requests" {
            upgrade = true;
        }
        if name == "script-src"
            && values
                .iter()
                .any(|v| v.eq_ignore_ascii_case("'strict-dynamic'"))
        {
            strict_dynamic_script = true;
            diagnostics.push(Diag::new("csp.strict_dynamic"));
        }
        if (name == "script-src" || name == "style-src")
            && values
                .iter()
                .any(|v| v.eq_ignore_ascii_case("'unsafe-inline'"))
            && values.iter().any(|v| {
                v.starts_with("'nonce-")
                    || v.starts_with("'sha256-")
                    || v.starts_with("'sha384-")
                    || v.starts_with("'sha512-")
            })
        {
            // CSP3：同时出现时浏览器忽略 'unsafe-inline'
            diagnostics.push(Diag::new("csp.unsafe_inline_ignored").param("name", name.clone()));
        }
        directives.push(Directive { name, values });
    }
    Policy {
        report_only,
        raw: raw.to_owned(),
        directives,
        upgrade,
        strict_dynamic_script,
        diagnostics,
    }
}

/// 解析后的来源表达式
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    None,
    SelfOrigin,
    UnsafeInline,
    /// 'unsafe-eval' 等与资源判定无关的关键字
    Other,
    Nonce(String),
    Hash(String),
    Scheme(String),
    Host {
        scheme: Option<String>,
        host: String,
        port: Option<u16>,
        path: Option<String>,
    },
}

pub fn parse_expr(raw: &str) -> Option<Expr> {
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "'none'" => return Some(Expr::None),
        "'self'" => return Some(Expr::SelfOrigin),
        "'unsafe-inline'" => return Some(Expr::UnsafeInline),
        "'unsafe-eval'" | "'wasm-unsafe-eval'" | "'unsafe-hashes'" | "'report-sample'"
        | "'strict-dynamic'" => return Some(Expr::Other),
        _ => {}
    }
    if lower.starts_with("'nonce-") {
        return Some(Expr::Nonce(lower));
    }
    if lower.starts_with("'sha256-")
        || lower.starts_with("'sha384-")
        || lower.starts_with("'sha512-")
    {
        return Some(Expr::Hash(lower));
    }
    // scheme-source：data: / blob:（冒号后为空）
    if let Some(scheme) = lower.strip_suffix(':')
        && !scheme.contains(':')
        && !scheme.contains('/')
    {
        return Some(Expr::Scheme(scheme.to_owned()));
    }
    // host-source：[scheme://] host [:port] [/path]
    let mut rest = lower.as_str();
    let mut scheme = None;
    if let Some((head, tail)) = rest.split_once(':')
        && tail.starts_with("//")
    {
        scheme = Some(head.to_owned());
        rest = tail.trim_start_matches('/');
    }
    if rest.is_empty() || rest.starts_with('/') {
        return None;
    }
    let (host_port, path) = match rest.find('/') {
        Some(at) => (&rest[..at], Some(rest[at..].to_owned())),
        None => (rest, None),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port_text)) => match port_text.parse::<u16>() {
            Ok(port) => (host.to_owned(), Some(port)),
            Err(_) => (host_port.to_owned(), None),
        },
        None => (host_port.to_owned(), None),
    };
    if host.is_empty() {
        return None;
    }
    Some(Expr::Host {
        scheme,
        host,
        port,
        path,
    })
}

/// 参与匹配的 URL 要素（端口已按 scheme 归一化为默认端口）
pub struct UrlParts {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn url_parts(url: &Url) -> UrlParts {
    UrlParts {
        scheme: url.scheme().to_owned(),
        host: url
            .host_str()
            .unwrap_or_default()
            .trim_end_matches('.')
            .to_ascii_lowercase(),
        port: url.port_or_known_default().unwrap_or(0),
        path: url.path().to_owned(),
    }
}

fn default_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        "ftp" => Some(21),
        _ => None,
    }
}

fn host_matches(pattern: &str, host: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        // 通配只匹配子域，不匹配裸域
        return host.ends_with(&format!(".{suffix}"));
    }
    pattern == host
}

fn path_matches(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() || pattern == "/" {
        return true;
    }
    if pattern.ends_with('/') {
        return path.starts_with(pattern);
    }
    path == pattern
}

/// 匹配上下文：URL 资源带 url；内联资源只有 nonce/hash
pub struct MatchCtx<'a> {
    pub document: Option<&'a Origin>,
    pub url: Option<&'a UrlParts>,
    pub nonce: Option<&'a str>,
    pub hash: Option<&'a str>,
}

/// 匹配一个来源表达式；`Err(())` 表示静态无法判定（例如缺少文档来源的 'self'）
pub fn matches(expr: &Expr, ctx: &MatchCtx<'_>) -> Result<bool, ()> {
    match expr {
        Expr::None => Ok(false),
        // 'unsafe-inline' 只对内联资源有意义
        Expr::UnsafeInline => Ok(ctx.url.is_none()),
        Expr::Other => Ok(false),
        Expr::Nonce(expected) => Ok(ctx
            .nonce
            .is_some_and(|nonce| nonce.eq_ignore_ascii_case(expected))),
        Expr::Hash(expected) => Ok(ctx
            .hash
            .is_some_and(|hash| hash.eq_ignore_ascii_case(expected))),
        Expr::Scheme(scheme) => Ok(ctx.url.is_some_and(|url| url.scheme == *scheme)),
        Expr::SelfOrigin => {
            let Some(url) = ctx.url else { return Ok(false) };
            let Some(origin) = ctx.document else {
                return Err(());
            };
            Ok(origin.host == url.host
                && origin.port == url.port
                && (origin.scheme == url.scheme
                    || (origin.scheme == "https" && url.scheme == "wss")))
        }
        Expr::Host {
            scheme,
            host,
            port,
            path,
        } => {
            let Some(url) = ctx.url else { return Ok(false) };
            if let Some(expr_scheme) = scheme
                && url.scheme != *expr_scheme
            {
                return Ok(false);
            }
            if !host_matches(host, &url.host) {
                return Ok(false);
            }
            match port {
                Some(expr_port) if *expr_port != url.port => return Ok(false),
                Some(_) => {}
                None => {
                    // 省略端口时匹配该 scheme 的默认端口
                    if default_port(&url.scheme).is_none_or(|default| default != url.port) {
                        return Ok(false);
                    }
                }
            }
            match path {
                Some(expr_path) => Ok(path_matches(expr_path, &url.path)),
                None => Ok(true),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

pub fn origin(url: &Url) -> Origin {
    Origin {
        scheme: url.scheme().to_owned(),
        host: url.host_str().unwrap_or_default().to_ascii_lowercase(),
        port: url.port_or_known_default().unwrap_or(0),
    }
}

/// 给一份片段式 URL（用户可能省略 scheme）补全 https://
pub fn parse_loose_url(text: &str) -> Result<Url, PluginError> {
    let trimmed = text.trim();
    let candidate = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    Url::parse(&candidate)
        .map_err(|e| PluginError::new("csp.invalid_url").with("detail", e.to_string()))
}

#[cfg(test)]
#[path = "policy_test.rs"]
mod tests;
