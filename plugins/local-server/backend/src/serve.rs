//! 静态文件服务任务：多个工作线程处理请求，每个请求写入实时日志，取消任务即停止服务。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};
use tiny_http::{Header, Response, Server, StatusCode};

use crate::addresses;
use crate::handler::{self, Body, Config, Reply};

const WORKERS: usize = 8;
const POLL: Duration = Duration::from_millis(200);
/// 日志中记录的 URL 最大长度
const MAX_URL: usize = 300;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub root: String,
    pub port: u16,
    #[serde(default)]
    pub lan: bool,
    #[serde(default = "yes")]
    pub listing: bool,
    #[serde(default)]
    pub spa: bool,
    #[serde(default)]
    pub cors: bool,
    #[serde(default = "yes")]
    pub hide_dotfiles: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub requests: u64,
    pub bytes: u64,
}

pub fn config(args: &Args) -> PluginResult<Config> {
    let root = PathBuf::from(&args.root)
        .canonicalize()
        .ok()
        .filter(|p| p.is_dir())
        .ok_or_else(|| PluginError::new("server.not_directory").with("path", args.root.clone()))?;
    Ok(Config {
        root,
        listing: args.listing,
        spa: args.spa,
        cors: args.cors,
        hide_dotfiles: args.hide_dotfiles,
    })
}

fn body_reader(body: &Body) -> std::io::Result<Box<dyn Read + Send>> {
    Ok(match body {
        Body::Empty => Box::new(std::io::empty()),
        Body::Text(text) => Box::new(std::io::Cursor::new(text.clone().into_bytes())),
        Body::File(path, start, len) => {
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(*start))?;
            Box::new(file.take(*len))
        }
    })
}

fn to_response(reply: &Reply) -> Response<Box<dyn Read + Send>> {
    let (status, reader) = match body_reader(&reply.body) {
        Ok(reader) => (reply.status, reader),
        // 文件在处理过程中被删除或无法读取
        Err(_) => (500, Box::new(std::io::empty()) as Box<dyn Read + Send>),
    };
    let length = if status == reply.status {
        reply.length()
    } else {
        0
    };
    let headers = reply
        .headers
        .iter()
        .filter_map(|(k, v)| Header::from_bytes(k.as_bytes(), v.as_bytes()).ok())
        .collect();
    Response::new(
        StatusCode(status),
        headers,
        reader,
        Some(length as usize),
        None,
    )
}

fn short(url: &str) -> String {
    if url.len() <= MAX_URL {
        return url.to_owned();
    }
    let mut end = MAX_URL;
    while !url.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &url[..end])
}

pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Summary> {
    let config = config(&args)?;
    let listener = addresses::bind(args.port, args.lan)?;
    let server = Server::from_listener(listener, None).map_err(|e| {
        PluginError::new("server.bind_failed")
            .with("port", args.port)
            .with("detail", e.to_string())
    })?;
    ctx.stage("serving");
    ctx.log(
        LogLevel::Info,
        "server.started",
        json!({ "root": config.root.to_string_lossy(), "port": args.port, "lan": args.lan }),
    );

    let requests = AtomicU64::new(0);
    let bytes = AtomicU64::new(0);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| {
                while !ctx.is_cancelled() {
                    let request = match server.recv_timeout(POLL) {
                        Ok(Some(request)) => request,
                        Ok(None) => continue,
                        Err(_) => break,
                    };
                    let started = Instant::now();
                    let method = request.method().as_str().to_owned();
                    let url = request.url().to_owned();
                    let range = request
                        .headers()
                        .iter()
                        .find(|h| h.field.equiv("Range"))
                        .map(|h| h.value.as_str().to_owned());
                    let remote = request
                        .remote_addr()
                        .map(|a| a.ip().to_string())
                        .unwrap_or_default();
                    let reply = handler::handle(&config, &method, &url, range.as_deref());
                    let response = to_response(&reply);
                    let status = response.status_code().0;
                    let sent = if method == "HEAD" { 0 } else { reply.length() };
                    // 客户端中途断开不算服务错误
                    let _ = request.respond(response);
                    requests.fetch_add(1, Ordering::Relaxed);
                    bytes.fetch_add(sent, Ordering::Relaxed);
                    let level = match status {
                        500.. => LogLevel::Error,
                        400.. => LogLevel::Warn,
                        _ => LogLevel::Info,
                    };
                    ctx.log(
                        level,
                        "server.request",
                        json!({
                            "method": method,
                            "url": short(&url),
                            "status": status,
                            "size": sent,
                            "ms": started.elapsed().as_millis() as u64,
                            "remote": remote,
                        }),
                    );
                }
            });
        }
    });
    drop(server);
    let summary = Summary {
        requests: requests.into_inner(),
        bytes: bytes.into_inner(),
    };
    ctx.log(
        LogLevel::Info,
        "server.stopped",
        json!({ "requests": summary.requests, "bytes": summary.bytes }),
    );
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "serve_test.rs"]
mod tests;
