//! 解析 URL：拆出各部分并解码，识别主机类型、国际化域名与跟踪参数。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};
use url::{Host, Url};

use crate::tracking;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParamInfo {
    pub key: String,
    pub value: String,
    /// 原始（未解码）写法
    pub raw: String,
    pub tracking: bool,
    /// 同名参数出现了多次
    pub duplicate: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HostKind {
    Domain,
    Ipv4,
    Ipv6,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    /// 规范化后的 URL
    pub href: String,
    /// 解码后便于阅读的 URL
    pub readable: String,
    /// 输入没有协议，按 https 解析
    pub assumed_scheme: bool,
    pub scheme: String,
    pub username: String,
    pub password: Option<String>,
    pub host: Option<String>,
    pub host_kind: Option<HostKind>,
    /// 国际化域名的 Unicode 写法（与 host 不同时才有）
    pub host_unicode: Option<String>,
    pub port: Option<u16>,
    /// 该协议的默认端口
    pub default_port: Option<u16>,
    pub origin: String,
    pub path: String,
    pub segments: Vec<String>,
    pub query: Option<String>,
    pub params: Vec<ParamInfo>,
    pub fragment: Option<String>,
}

fn decode(text: &str) -> String {
    percent_encoding::percent_decode_str(text)
        .decode_utf8_lossy()
        .into_owned()
}

/// 查询串里 + 表示空格
fn decode_query(text: &str) -> String {
    decode(&text.replace('+', " "))
}

/// 没有协议时补上 https://（如 example.com/path）
fn with_scheme(input: &str) -> (String, bool) {
    let has_scheme = input.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
                && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
                // localhost:8080 这样的写法不是协议
                && !input[scheme.len() + 1..].starts_with(|c: char| c.is_ascii_digit())
    });
    if has_scheme {
        (input.to_owned(), false)
    } else {
        (format!("https://{}", input.trim_start_matches("//")), true)
    }
}

pub fn url(input: &str) -> PluginResult<(Url, bool)> {
    let input = input.trim();
    if input.is_empty() {
        return Err(PluginError::new("url.empty"));
    }
    let (text, assumed) = with_scheme(input);
    Url::parse(&text)
        .map(|u| (u, assumed))
        .map_err(|e| PluginError::new("url.invalid").with("detail", e.to_string()))
}

pub fn params(url: &Url) -> Vec<ParamInfo> {
    let Some(query) = url.query() else {
        return Vec::new();
    };
    let pairs: Vec<(String, String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|raw| {
            let (k, v) = raw.split_once('=').unwrap_or((raw, ""));
            (decode_query(k), decode_query(v), raw.to_owned())
        })
        .collect();
    pairs
        .iter()
        .map(|(key, value, raw)| ParamInfo {
            key: key.clone(),
            value: value.clone(),
            raw: raw.clone(),
            tracking: tracking::is_tracking(key),
            duplicate: pairs.iter().filter(|(k, _, _)| k == key).count() > 1,
        })
        .collect()
}

/// 常见协议的默认端口
fn default_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        "ftp" => Some(21),
        _ => None,
    }
}

pub fn parse(input: &str) -> PluginResult<Parsed> {
    let (url, assumed_scheme) = url(input)?;
    let (host, host_kind, host_unicode) = match url.host() {
        Some(Host::Domain(domain)) => {
            let (unicode, _) = idna::domain_to_unicode(domain);
            let differs = unicode != domain;
            (
                Some(domain.to_owned()),
                Some(HostKind::Domain),
                differs.then_some(unicode),
            )
        }
        Some(Host::Ipv4(ip)) => (Some(ip.to_string()), Some(HostKind::Ipv4), None),
        Some(Host::Ipv6(ip)) => (Some(ip.to_string()), Some(HostKind::Ipv6), None),
        None => (None, None, None),
    };
    let segments = url
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).map(decode).collect())
        .unwrap_or_default();
    let readable = {
        let mut text = format!("{}:", url.scheme());
        if url.has_host() {
            text.push_str("//");
            if !url.username().is_empty() {
                text.push_str(&decode(url.username()));
                text.push('@');
            }
            text.push_str(
                host_unicode
                    .as_deref()
                    .or(host.as_deref())
                    .unwrap_or_default(),
            );
            if let Some(port) = url.port() {
                text.push_str(&format!(":{port}"));
            }
        }
        text.push_str(&decode(url.path()));
        if let Some(q) = url.query() {
            text.push('?');
            text.push_str(&decode_query(q));
        }
        if let Some(f) = url.fragment() {
            text.push('#');
            text.push_str(&decode(f));
        }
        text
    };
    Ok(Parsed {
        href: url.to_string(),
        readable,
        assumed_scheme,
        scheme: url.scheme().to_owned(),
        username: decode(url.username()),
        password: url.password().map(decode),
        host,
        host_kind,
        host_unicode,
        port: url.port(),
        default_port: default_port(url.scheme()),
        origin: url.origin().ascii_serialization(),
        path: decode(url.path()),
        segments,
        query: url.query().map(str::to_owned),
        params: params(&url),
        fragment: url.fragment().map(decode),
    })
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
