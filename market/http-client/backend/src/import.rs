use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};
use tfm_curl_converter::{Body as CurlBody, Warning, parse_command_with_options};

use crate::http::{Body, BodyKind, Pair, Request};
use crate::workspace::validate_request;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportArgs {
    pub command: String,
}

#[derive(Debug, Serialize)]
pub struct Imported {
    pub request: Request,
    pub warnings: Vec<Warning>,
}

fn warning(code: &str) -> Warning {
    Warning {
        code: code.to_owned(),
        params: Default::default(),
    }
}

pub fn import(command: &str) -> PluginResult<Imported> {
    let (parsed, mut warnings, options) = parse_command_with_options(command)?;
    if !matches!(
        parsed.method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        return Err(PluginError::new("http.curl_method_unsupported").with("method", parsed.method));
    }
    if warnings.iter().any(|item| {
        matches!(
            item.code.as_str(),
            "curl.upload_unsupported"
                | "curl.data_file_mixed"
                | "curl.urlencode_file"
                | "curl.cookie_file"
        )
    }) {
        return Err(PluginError::new("http.curl_file_unsupported"));
    }
    for option in options {
        let represented = matches!(
            option.as_str(),
            "--request"
                | "--header"
                | "--data"
                | "--data-ascii"
                | "--data-binary"
                | "--data-raw"
                | "--json"
                | "--data-urlencode"
                | "--user"
                | "--oauth2-bearer"
                | "--cookie"
                | "--user-agent"
                | "--referer"
                | "--max-time"
                | "--proxy"
                | "--url"
                | "--insecure"
                | "--location"
                | "--compressed"
                | "--head"
                | "--get"
                | "--form"
                | "--form-string"
        );
        let display_only = matches!(
            option.as_str(),
            "--silent"
                | "--show-error"
                | "--verbose"
                | "--include"
                | "--fail"
                | "--fail-with-body"
                | "--progress-bar"
                | "--no-progress-meter"
                | "--remote-name"
                | "--remote-name-all"
                | "--remote-header-name"
                | "--no-buffer"
                | "--create-dirs"
                | "--output"
                | "--write-out"
                | "--dump-header"
                | "--cookie-jar"
                | "--trace"
                | "--trace-ascii"
                | "--stderr"
                | "-s"
                | "-S"
                | "-v"
                | "-i"
                | "-f"
                | "-O"
                | "-#"
                | "-N"
                | "-R"
                | "-J"
        );
        let already_warned = warnings.iter().any(|item| {
            item.params
                .get("option")
                .and_then(serde_json::Value::as_str)
                == Some(option.as_str())
        });
        if !represented && !display_only && !already_warned {
            let mut item = warning("http.curl_option_unsupported");
            item.params.insert("option".into(), option.into());
            warnings.push(item);
        }
    }
    let body = match &parsed.body {
        CurlBody::None => Body::default(),
        CurlBody::Text { text } => Body {
            kind: if parsed
                .header("content-type")
                .is_some_and(|value| value.to_ascii_lowercase().contains("json"))
                && serde_json::from_str::<serde_json::Value>(text).is_ok()
            {
                BodyKind::Json
            } else {
                BodyKind::Text
            },
            text: text.clone(),
            form: Vec::new(),
        },
        CurlBody::File { .. } | CurlBody::Multipart { .. } => {
            return Err(PluginError::new("http.curl_body_unsupported"));
        }
    };
    if parsed.insecure {
        warnings.push(warning("http.curl_insecure"));
    }
    let use_proxy = match &parsed.proxy {
        Some(proxy) if proxy.is_empty() => false,
        Some(_) => {
            warnings.push(warning("http.curl_proxy"));
            true
        }
        None => true,
    };
    if parsed.compressed {
        warnings.push(warning("http.curl_compressed"));
    }
    let timeout_ms = parsed.timeout.map_or(30_000, |seconds| {
        let milliseconds = (seconds * 1000.0).round() as u64;
        let adjusted = milliseconds.clamp(1000, 600_000);
        if milliseconds != adjusted {
            warnings.push(warning("http.curl_timeout_adjusted"));
        }
        adjusted
    });
    // 明确请求事件流的命令导入后直接用流式接收
    let stream = parsed.headers.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case("accept")
            && value.to_ascii_lowercase().contains("text/event-stream")
    });
    let request = Request {
        method: parsed.method,
        url: parsed.url,
        params: Vec::new(),
        headers: parsed
            .headers
            .into_iter()
            .map(|(key, value)| Pair {
                key,
                value,
                enabled: true,
            })
            .collect(),
        body,
        timeout_ms,
        follow_redirects: parsed.follow_redirects,
        use_proxy,
        stream,
    };
    validate_request(&request)?;
    crate::http::normalize_url(&request.url)?;
    Ok(Imported { request, warnings })
}

#[cfg(test)]
#[path = "import_test.rs"]
mod tests;
