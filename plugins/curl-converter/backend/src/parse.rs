//! 解析 curl 参数为请求模型，行为尽量与 curl 一致：
//! 多个 `-d` 用 `&` 连接、带请求体时默认 POST 与表单类型、`-G` 把数据放进查询串等。

use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub name: String,
    /// 文本字段的值
    pub value: Option<String>,
    /// 文件字段的路径
    pub file: Option<String>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Body {
    None,
    Text {
        text: String,
    },
    /// `-d @file`：请求体来自文件
    File {
        path: String,
    },
    Multipart {
        parts: Vec<Part>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Body,
    pub insecure: bool,
    pub follow_redirects: bool,
    pub compressed: bool,
    /// 秒
    pub timeout: Option<f64>,
    pub proxy: Option<String>,
}

/// 无法完整转换的内容，界面按错误码翻译
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Warning {
    pub code: String,
    pub params: serde_json::Map<String, serde_json::Value>,
}

impl Warning {
    fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            params: Default::default(),
        }
    }

    fn with(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn set_default_header(&mut self, name: &str, value: &str) {
        if self.header(name).is_none() {
            self.headers.push((name.to_owned(), value.to_owned()));
        }
    }
}

/// 带参数的短选项
const SHORT_WITH_VALUE: &str = "XHdFubAemxowDcETKrzYyCQtP";
/// 需要值但转换时不使用的长选项
const IGNORED_WITH_VALUE: &[&str] = &[
    "--output",
    "--write-out",
    "--dump-header",
    "--cookie-jar",
    "--retry",
    "--retry-delay",
    "--retry-max-time",
    "--max-redirs",
    "--resolve",
    "--connect-to",
    "--cacert",
    "--capath",
    "--cert",
    "--cert-type",
    "--key",
    "--key-type",
    "--ciphers",
    "--interface",
    "--limit-rate",
    "--range",
    "--trace",
    "--trace-ascii",
    "--stderr",
    "--config",
    "--time-cond",
    "--speed-limit",
    "--speed-time",
    "--keepalive-time",
    "--proxy-user",
    "--noproxy",
    "--dns-servers",
    "--local-port",
    "--quote",
    "--continue-at",
];
/// 不影响请求内容、直接忽略的开关
const IGNORED_FLAGS: &[&str] = &[
    "--silent",
    "--show-error",
    "--verbose",
    "--include",
    "--fail",
    "--fail-with-body",
    "--progress-bar",
    "--no-progress-meter",
    "--remote-name",
    "--remote-name-all",
    "--remote-header-name",
    "--no-buffer",
    "--http1.0",
    "--http1.1",
    "--http2",
    "--http2-prior-knowledge",
    "--http3",
    "--tlsv1",
    "--tlsv1.0",
    "--tlsv1.1",
    "--tlsv1.2",
    "--tlsv1.3",
    "--ipv4",
    "--ipv6",
    "--globoff",
    "--path-as-is",
    "--create-dirs",
    "--tcp-nodelay",
    "--no-keepalive",
    "--raw",
    "--ssl",
    "--ssl-reqd",
    "--disable",
];
const IGNORED_SHORT: &str = "sSvifO#N0123460gqRjJlZ";

fn short_long(flag: char) -> Option<&'static str> {
    Some(match flag {
        'X' => "--request",
        'H' => "--header",
        'd' => "--data",
        'F' => "--form",
        'u' => "--user",
        'b' => "--cookie",
        'A' => "--user-agent",
        'e' => "--referer",
        'm' => "--max-time",
        'x' => "--proxy",
        'k' => "--insecure",
        'L' => "--location",
        'I' => "--head",
        'G' => "--get",
        'T' => "--upload-file",
        'o' => "--output",
        'w' => "--write-out",
        'D' => "--dump-header",
        'c' => "--cookie-jar",
        'E' => "--cert",
        'K' => "--config",
        'r' => "--range",
        'z' => "--time-cond",
        'Y' => "--speed-limit",
        'y' => "--speed-time",
        'C' => "--continue-at",
        'Q' => "--quote",
        't' => "--telnet-option",
        'P' => "--ftp-port",
        _ => return None,
    })
}

fn takes_value(long: &str) -> bool {
    matches!(
        long,
        "--request"
            | "--header"
            | "--data"
            | "--data-raw"
            | "--data-ascii"
            | "--data-binary"
            | "--data-urlencode"
            | "--json"
            | "--form"
            | "--form-string"
            | "--user"
            | "--cookie"
            | "--user-agent"
            | "--referer"
            | "--max-time"
            | "--connect-timeout"
            | "--proxy"
            | "--url"
            | "--upload-file"
            | "--oauth2-bearer"
            | "--telnet-option"
            | "--ftp-port"
    ) || IGNORED_WITH_VALUE.contains(&long)
}

/// 与 curl `--data-urlencode` 相同：保留字母数字与 `-._~`，空格编码为 `+`，其余按 UTF-8 编码为小写的 `%xx`
pub fn urlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else if byte == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{byte:02x}"));
        }
    }
    out
}

enum Data {
    Text(String),
    File(String),
}

fn missing(option: &str) -> PluginError {
    PluginError::new("curl.missing_value").with("option", option)
}

/// `-F name=value`、`name=@file;type=…;filename=…`、`name=<file`
fn form_part(spec: &str, literal: bool) -> Option<Part> {
    let (name, rest) = spec.split_once('=')?;
    let mut part = Part {
        name: name.to_owned(),
        value: None,
        file: None,
        filename: None,
        content_type: None,
    };
    if literal {
        part.value = Some(rest.to_owned());
        return Some(part);
    }
    if let Some(file) = rest.strip_prefix('@').or_else(|| rest.strip_prefix('<')) {
        let mut pieces = file.split(';');
        part.file = pieces.next().map(str::to_owned);
        for piece in pieces {
            match piece.split_once('=') {
                Some(("type", v)) => part.content_type = Some(v.to_owned()),
                Some(("filename", v)) => part.filename = Some(v.trim_matches('"').to_owned()),
                _ => {}
            }
        }
    } else {
        let mut pieces = rest.split(";type=");
        part.value = pieces.next().map(str::to_owned);
        part.content_type = pieces.next().map(str::to_owned);
    }
    Some(part)
}

pub fn parse(tokens: &[String]) -> PluginResult<(Request, Vec<Warning>)> {
    let mut args = tokens.iter().map(String::as_str).peekable();
    if args.peek().is_some_and(|first| {
        let name = first.rsplit(['/', '\\']).next().unwrap_or(first);
        name.eq_ignore_ascii_case("curl") || name.eq_ignore_ascii_case("curl.exe")
    }) {
        args.next();
    }

    let mut warnings = Vec::new();
    let mut urls: Vec<String> = Vec::new();
    let mut method: Option<String> = None;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut data: Vec<Data> = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    let (mut insecure, mut follow, mut compressed, mut head, mut get) =
        (false, false, false, false, false);
    let mut json = false;
    let mut timeout = None;
    let mut proxy = None;
    let mut basic: Option<String> = None;

    // 展开组合短选项（-sSL、-XPOST）为 (长选项, 值)
    let mut options: Vec<(String, Option<String>)> = Vec::new();
    let mut positional = false;
    while let Some(arg) = args.next() {
        if positional || !arg.starts_with('-') || arg == "-" {
            urls.push(arg.to_owned());
            continue;
        }
        if arg == "--" {
            positional = true;
            continue;
        }
        if let Some(long) = arg.strip_prefix("--") {
            let long = format!("--{long}");
            if takes_value(&long) {
                let value = args.next().ok_or_else(|| missing(&long))?;
                options.push((long, Some(value.to_owned())));
            } else {
                options.push((long, None));
            }
            continue;
        }
        let flags: Vec<char> = arg[1..].chars().collect();
        for (i, flag) in flags.iter().enumerate() {
            if SHORT_WITH_VALUE.contains(*flag) {
                let attached: String = flags[i + 1..].iter().collect();
                let value = if attached.is_empty() {
                    args.next()
                        .ok_or_else(|| missing(&format!("-{flag}")))?
                        .to_owned()
                } else {
                    attached
                };
                let long = short_long(*flag).unwrap_or("--unknown").to_owned();
                options.push((long, Some(value)));
                break;
            }
            match short_long(*flag) {
                Some(long) => options.push((long.to_owned(), None)),
                None if IGNORED_SHORT.contains(*flag) => {}
                None => warnings
                    .push(Warning::new("curl.unknown_option").with("option", format!("-{flag}"))),
            }
        }
    }

    for (option, value) in options {
        let value = value.unwrap_or_default();
        match option.as_str() {
            "--request" => method = Some(value.to_uppercase()),
            "--header" => {
                if let Some((name, v)) = value.split_once(':') {
                    headers.push((name.trim().to_owned(), v.trim().to_owned()));
                } else if let Some(name) = value.strip_suffix(';') {
                    // `-H "X-Empty;"` 发送空值的请求头
                    headers.push((name.trim().to_owned(), String::new()));
                }
            }
            "--data" | "--data-ascii" | "--data-binary" => match value.strip_prefix('@') {
                Some(path) => data.push(Data::File(path.to_owned())),
                None => data.push(Data::Text(if option == "--data-binary" {
                    value
                } else {
                    // curl 会去掉 -d 内容里的换行
                    value.replace(['\r', '\n'], "")
                })),
            },
            "--data-raw" => data.push(Data::Text(value)),
            "--json" => {
                json = true;
                match value.strip_prefix('@') {
                    Some(path) => data.push(Data::File(path.to_owned())),
                    None => data.push(Data::Text(value)),
                }
            }
            "--data-urlencode" => {
                let encoded = match value.split_once('=') {
                    Some(("", content)) => urlencode(content),
                    Some((name, content)) => format!("{name}={}", urlencode(content)),
                    None if value.contains('@') => {
                        warnings.push(
                            Warning::new("curl.urlencode_file").with("value", value.as_str()),
                        );
                        continue;
                    }
                    None => urlencode(&value),
                };
                data.push(Data::Text(encoded));
            }
            "--form" | "--form-string" => match form_part(&value, option == "--form-string") {
                Some(part) => parts.push(part),
                None => warnings.push(Warning::new("curl.invalid_form").with("value", value)),
            },
            "--user" => basic = Some(value),
            "--oauth2-bearer" => headers.push(("Authorization".into(), format!("Bearer {value}"))),
            "--cookie" => {
                if value.contains('=') {
                    headers.push(("Cookie".into(), value));
                } else {
                    warnings.push(Warning::new("curl.cookie_file").with("path", value));
                }
            }
            "--user-agent" => headers.push(("User-Agent".into(), value)),
            "--referer" => headers.push(("Referer".into(), value)),
            "--max-time" => timeout = value.parse::<f64>().ok().filter(|t| *t > 0.0),
            "--connect-timeout" => {}
            "--proxy" => proxy = Some(value),
            "--url" => urls.push(value),
            "--insecure" => insecure = true,
            "--location" | "--location-trusted" => follow = true,
            "--compressed" => compressed = true,
            "--head" => head = true,
            "--get" => get = true,
            "--upload-file" => {
                warnings.push(Warning::new("curl.upload_unsupported").with("path", value))
            }
            other if IGNORED_FLAGS.contains(&other) || IGNORED_WITH_VALUE.contains(&other) => {}
            other => warnings.push(Warning::new("curl.unknown_option").with("option", other)),
        }
    }

    let mut url = urls
        .first()
        .cloned()
        .ok_or_else(|| PluginError::new("curl.no_url"))?;
    if urls.len() > 1 {
        warnings.push(Warning::new("curl.multiple_urls").with("count", urls.len()));
    }
    if !url.contains("://") {
        url = format!("http://{url}");
    }

    let body = if !parts.is_empty() {
        if !data.is_empty() {
            warnings.push(Warning::new("curl.data_and_form"));
        }
        Body::Multipart { parts }
    } else if let [Data::File(path)] = data.as_slice() {
        Body::File { path: path.clone() }
    } else if data.is_empty() {
        Body::None
    } else {
        let mut joined = Vec::new();
        for item in &data {
            match item {
                Data::Text(text) => joined.push(text.clone()),
                Data::File(path) => {
                    warnings.push(Warning::new("curl.data_file_mixed").with("path", path.as_str()));
                    joined.push(format!("@{path}"));
                }
            }
        }
        Body::Text {
            text: joined.join("&"),
        }
    };

    let mut request = Request {
        method: String::new(),
        url,
        headers,
        body,
        insecure,
        follow_redirects: follow,
        compressed,
        timeout,
        proxy,
    };

    // -G：数据改为查询参数
    if get && let Body::Text { text } = &request.body {
        let separator = if request.url.contains('?') { '&' } else { '?' };
        request.url = format!("{}{separator}{text}", request.url);
        request.body = Body::None;
    }

    if let Some(user) = basic {
        let credentials = if user.contains(':') {
            user
        } else {
            format!("{user}:")
        };
        request.headers.push((
            "Authorization".into(),
            format!(
                "Basic {}",
                data_encoding::BASE64.encode(credentials.as_bytes())
            ),
        ));
    }
    if json {
        request.set_default_header("Content-Type", "application/json");
        request.set_default_header("Accept", "application/json");
    }
    if matches!(request.body, Body::Text { .. } | Body::File { .. }) {
        request.set_default_header("Content-Type", "application/x-www-form-urlencoded");
    }

    request.method = method.unwrap_or_else(|| {
        if head {
            "HEAD"
        } else if matches!(request.body, Body::None) {
            "GET"
        } else {
            "POST"
        }
        .to_owned()
    });
    Ok((request, warnings))
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod tests;
