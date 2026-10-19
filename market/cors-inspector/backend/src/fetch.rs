//! Fetch CORS 规则常量与请求头清单。
//! 规范依据：WHATWG Fetch Standard 的 CORS protocol 章节（whatwg/fetch main，2026-10 快照）。
//! 未覆盖的浏览器行为（重定向、预检缓存、PNA、SameSite）由前端文案明确标注，不在判定内。

use tf_plugin_api::PluginError;

/// 每个请求/响应块的头部条数上限
pub const MAX_HEADERS: usize = 64;
/// 单个头部值长度上限
pub const MAX_VALUE: usize = 8 * 1024;
/// 头部名称长度上限
pub const MAX_NAME: usize = 128;

/// CORS-safelisted method（Fetch §2.2.3）：大小写敏感的精确匹配
pub const SAFELISTED_METHODS: &[&str] = &["GET", "HEAD", "POST"];

/// CORS-safelisted request-header 名称（Fetch §2.2.4，小写）
pub const SAFELISTED_REQUEST_NAMES: &[&str] = &[
    "accept",
    "accept-language",
    "content-language",
    "content-type",
    "range",
];

/// CORS-non-wildcard request-header name（Fetch §2.2.6）：`*` 通配符不覆盖它
pub const NON_WILDCARD_NAME: &str = "authorization";

/// forbidden request-header name（Fetch §2.2.2，小写；另有 proxy- / sec- 前缀）
const FORBIDDEN_REQUEST_NAMES: &[&str] = &[
    "accept-charset",
    "accept-encoding",
    "access-control-request-headers",
    "access-control-request-method",
    "connection",
    "content-length",
    "cookie",
    "cookie2",
    "date",
    "dnt",
    "expect",
    "host",
    "keep-alive",
    "origin",
    "referer",
    "set-cookie",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "via",
];

/// forbidden response-header name（Fetch §2.2.1）：任何情况下脚本都读不到
pub const FORBIDDEN_RESPONSE_NAMES: &[&str] = &["set-cookie", "set-cookie2"];

/// CORS-safelisted response-header name（Fetch §2.2.7，小写）
pub const SAFELISTED_RESPONSE_NAMES: &[&str] = &[
    "cache-control",
    "content-language",
    "content-length",
    "content-type",
    "expires",
    "last-modified",
    "pragma",
];

/// CORS-unsafe request-header byte（Fetch §2.2.5-1）
fn unsafe_request_byte(byte: u8) -> bool {
    matches!(byte,
        0x00..=0x08
        | 0x10..=0x19
        | 0x22 // "
        | 0x28 | 0x29 // ( )
        | 0x3A // :
        | 0x3C | 0x3E // < >
        | 0x3F // ?
        | 0x40 // @
        | 0x5B | 0x5C | 0x5D // [ \ ]
        | 0x7B | 0x7D // { }
        | 0x7F
    )
}

/// 头部值是否只含 safelist 允许的字节
fn value_bytes_safelisted(value: &str) -> bool {
    !value.bytes().any(unsafe_request_byte)
}

/// Content-Type 的 MIME essence 是否 safelisted（Fetch §2.2.5-3）
fn content_type_safelisted(value: &str) -> bool {
    // essence 取 ';' 前的部分；逗号意味着多个值合并，必然不是合法的单一类型
    let essence = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(
        essence.as_str(),
        "application/x-www-form-urlencoded" | "multipart/form-data" | "text/plain"
    )
}

/// Range 是否为 single byte range（Fetch §2.2.5-4）：`bytes=<start>-` 或 `bytes=<start>-<end>`
fn range_safelisted(value: &str) -> bool {
    let Some(range) = value.trim().strip_prefix("bytes=") else {
        return false;
    };
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };
    let Ok(start) = start.trim().parse::<u64>() else {
        return false;
    };
    match end.trim().parse::<u64>() {
        Ok(end) => end >= start,
        Err(_) => end.trim().is_empty(),
    }
}

/// (name, value) 是否为 CORS-safelisted request-header（Fetch §2.2.5）
pub fn is_safelisted_request_header(name: &str, value: &str) -> bool {
    if !SAFELISTED_REQUEST_NAMES.contains(&name) {
        return false;
    }
    match name {
        "content-type" => content_type_safelisted(value),
        "range" => range_safelisted(value),
        _ => value_bytes_safelisted(value),
    }
}

/// 是否为 forbidden request-header name（Fetch §2.2.2，含 proxy- / sec- 前缀）
pub fn is_forbidden_request_name(name: &str) -> bool {
    FORBIDDEN_REQUEST_NAMES.contains(&name)
        || name.starts_with("proxy-")
        || name.starts_with("sec-")
}

pub fn is_forbidden_response_name(name: &str) -> bool {
    FORBIDDEN_RESPONSE_NAMES.contains(&name)
}

/// HTTP 头部名称是否为合法 token（RFC 9110 tchar）
pub fn is_header_name_token(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

pub fn is_safelisted_method(method: &str) -> bool {
    SAFELISTED_METHODS.contains(&method)
}

/// 输入的头部条目（前端传 name/value）
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HeaderArg {
    pub name: String,
    #[serde(default)]
    pub value: String,
}

/// 归一化后的头部清单：名称小写、值去除首尾空白
#[derive(Debug)]
pub struct HeaderList {
    entries: Vec<(String, String)>,
}

impl HeaderList {
    /// 从输入构建；非法名称、超限直接报错，保证报告里的判定都基于合法输入
    pub fn build(args: &[HeaderArg]) -> Result<Self, PluginError> {
        if args.len() > MAX_HEADERS {
            return Err(PluginError::new("cors.header_too_many").with("max", MAX_HEADERS));
        }
        let mut entries = Vec::with_capacity(args.len());
        for arg in args {
            let name = arg.name.trim().to_owned();
            if name.len() > MAX_NAME || !is_header_name_token(&name) {
                return Err(PluginError::new("cors.invalid_header_name").with("name", name));
            }
            let value = arg.value.trim();
            if value.len() > MAX_VALUE {
                return Err(PluginError::new("cors.header_too_large").with("kb", MAX_VALUE >> 10));
            }
            entries.push((name.to_ascii_lowercase(), value.to_owned()));
        }
        Ok(Self { entries })
    }

    /// 同名头部的全部值（同名多次出现会被浏览器合并为 `, ` 连接）
    pub fn get(&self, name: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// 全部小写名称（按出现顺序去重）
    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (name, _) in &self.entries {
            if !out.iter().any(|seen: &String| seen == name) {
                out.push(name.clone());
            }
        }
        out
    }

    /// 触发预检的名称：不在 safelist 内的请求头（去重、保持出现顺序、小写）
    pub fn unsafe_names(&self) -> Vec<String> {
        self.names()
            .into_iter()
            .filter(|name| {
                !self
                    .get(name)
                    .iter()
                    .all(|v| is_safelisted_request_header(name, v))
            })
            .collect()
    }

    /// forbidden request-header names（去重、保持出现顺序、小写）
    pub fn forbidden_names(&self) -> Vec<String> {
        self.names()
            .into_iter()
            .filter(|name| is_forbidden_request_name(name))
            .collect()
    }
}

/// 头部参数列表的公共校验入口（附加上下文诊断）
pub fn build_headers(args: &[HeaderArg]) -> Result<HeaderList, PluginError> {
    HeaderList::build(args)
}

/// 响应头块输入：状态 + 头部
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseArg {
    #[serde(default)]
    pub status: Option<i64>,
    #[serde(default)]
    pub headers: Vec<HeaderArg>,
}

/// 非法的响应状态输入
pub fn validate_status(status: Option<i64>, code: &str) -> Result<(), PluginError> {
    match status {
        None => Ok(()),
        Some(value) if (100..=999).contains(&value) => Ok(()),
        Some(value) => Err(PluginError::new(code).with("status", value)),
    }
}

/// 展示用的头部视图：敏感头的值默认遮罩
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeaderView {
    pub name: String,
    pub value: String,
    pub masked: bool,
}

impl HeaderView {
    pub fn new(name: &str, value: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        let masked = matches!(
            lower.as_str(),
            "cookie" | "cookie2" | "authorization" | "proxy-authorization" | "set-cookie"
        );
        Self {
            name: name.to_owned(),
            value: if masked {
                "••••••".to_owned()
            } else {
                value.to_owned()
            },
            masked,
        }
    }
}

#[cfg(test)]
#[path = "fetch_test.rs"]
mod tests;
