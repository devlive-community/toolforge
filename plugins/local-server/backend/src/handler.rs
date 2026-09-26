//! 请求处理：把（方法、路径、请求头）映射为响应描述，不涉及网络，便于测试。
//! 防止路径穿越；支持目录列表、index.html、SPA 回退、隐藏文件与 Range 请求。

use std::path::{Component, Path, PathBuf};

use percent_encoding::percent_decode_str;

#[derive(Debug, Clone)]
pub struct Config {
    /// 已规范化的根目录
    pub root: PathBuf,
    pub listing: bool,
    pub spa: bool,
    pub cors: bool,
    /// 隐藏以 . 开头的文件与目录
    pub hide_dotfiles: bool,
}

#[derive(Debug, PartialEq)]
pub enum Body {
    Empty,
    Text(String),
    /// 文件的某一段：(路径, 起始偏移, 长度)
    File(PathBuf, u64, u64),
}

#[derive(Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Reply {
    fn new(status: u16, body: Body) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body,
        }
    }

    fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    pub fn length(&self) -> u64 {
        match &self.body {
            Body::Empty => 0,
            Body::Text(text) => text.len() as u64,
            Body::File(_, _, len) => *len,
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page(status: u16, title: &str) -> Reply {
    let html = format!(
        "<!DOCTYPE html><meta charset=\"utf-8\"><title>{status} {title}</title>\
         <body style=\"font-family:system-ui;padding:2rem;color:#555\"><h1>{status}</h1><p>{title}</p>"
    );
    Reply::new(status, Body::Text(html)).header("Content-Type", "text/html; charset=utf-8")
}

/// 解码 URL 路径并拒绝 .. 等危险片段；返回相对根目录的安全路径
pub fn safe_relative(raw: &str) -> Option<PathBuf> {
    let path = raw.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode_str(path).decode_utf8().ok()?;
    if decoded.contains('\0') || decoded.contains('\\') {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in Path::new(decoded.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(relative)
}

fn content_type(path: &Path) -> String {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let text = mime.type_() == "text"
        || matches!(
            mime.subtype().as_str(),
            "javascript" | "json" | "xml" | "svg+xml"
        );
    if text {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

/// 解析单个 Range：`bytes=a-b`、`bytes=a-`、`bytes=-n`
pub fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || size == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (size.saturating_sub(n), size - 1)
        }
        (a, "") => (a.parse().ok()?, size - 1),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(size - 1)),
    };
    (start <= end && start < size).then_some((start, end))
}

fn hidden(relative: &Path) -> bool {
    relative
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

fn serve_file(path: &Path, range: Option<&str>) -> Reply {
    let Ok(metadata) = std::fs::metadata(path) else {
        return page(404, "Not Found");
    };
    let size = metadata.len();
    let reply = match range.map(|r| parse_range(r, size)) {
        Some(Some((start, end))) => {
            Reply::new(206, Body::File(path.to_owned(), start, end - start + 1))
                .header("Content-Range", format!("bytes {start}-{end}/{size}"))
        }
        Some(None) => {
            return Reply::new(416, Body::Empty).header("Content-Range", format!("bytes */{size}"));
        }
        None => Reply::new(200, Body::File(path.to_owned(), 0, size)),
    };
    reply
        .header("Content-Type", content_type(path))
        .header("Accept-Ranges", "bytes")
}

fn listing(dir: &Path, url_path: &str, config: &Config) -> Reply {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return page(403, "Forbidden");
    };
    let mut items: Vec<(bool, String, u64)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if config.hide_dotfiles && name.starts_with('.') {
                return None;
            }
            let meta = entry.metadata().ok()?;
            Some((meta.is_dir(), name, meta.len()))
        })
        .collect();
    items.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    let base = if url_path.ends_with('/') {
        url_path.to_owned()
    } else {
        format!("{url_path}/")
    };
    let mut rows = String::new();
    if base != "/" {
        rows.push_str("<li><a href=\"../\">../</a></li>");
    }
    for (is_dir, name, size) in &items {
        let href = percent_encoding::utf8_percent_encode(name, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
        let slash = if *is_dir { "/" } else { "" };
        let size = if *is_dir {
            String::new()
        } else {
            format!(" <small>{size} B</small>")
        };
        rows.push_str(&format!(
            "<li><a href=\"{href}{slash}\">{}{slash}</a>{size}</li>",
            escape(name)
        ));
    }
    let title = escape(&base);
    let html = format!(
        "<!DOCTYPE html><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\
         <title>{title}</title><body style=\"font-family:system-ui;padding:1.5rem;line-height:1.8\">\
         <h2>{title}</h2><ul style=\"list-style:none;padding:0\">{rows}</ul>"
    );
    Reply::new(200, Body::Text(html)).header("Content-Type", "text/html; charset=utf-8")
}

pub fn handle(config: &Config, method: &str, url: &str, range: Option<&str>) -> Reply {
    let reply = route(config, method, url, range);
    let reply = reply.header("Cache-Control", "no-cache");
    if config.cors {
        reply.header("Access-Control-Allow-Origin", "*")
    } else {
        reply
    }
}

fn route(config: &Config, method: &str, url: &str, range: Option<&str>) -> Reply {
    if config.cors && method == "OPTIONS" {
        return Reply::new(204, Body::Empty)
            .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
            .header("Access-Control-Allow-Headers", "*");
    }
    if method != "GET" && method != "HEAD" {
        return page(405, "Method Not Allowed").header("Allow", "GET, HEAD");
    }
    let Some(relative) = safe_relative(url) else {
        return page(400, "Bad Request");
    };
    if config.hide_dotfiles && hidden(&relative) {
        return page(404, "Not Found");
    }
    let target = config.root.join(&relative);
    // 规范化后仍须位于根目录内（防止符号链接指向外部）
    let resolved = match target.canonicalize() {
        Ok(path) if path.starts_with(&config.root) => Some(path),
        Ok(_) => return page(403, "Forbidden"),
        Err(_) => None,
    };
    let url_path = url.split(['?', '#']).next().unwrap_or("/");
    match resolved {
        Some(path) if path.is_dir() => {
            if !url_path.ends_with('/') {
                return Reply::new(301, Body::Empty).header("Location", format!("{url_path}/"));
            }
            let index = path.join("index.html");
            if index.is_file() {
                return serve_file(&index, range);
            }
            if config.listing {
                return listing(&path, url_path, config);
            }
            page(403, "Forbidden")
        }
        Some(path) => serve_file(&path, range),
        None => {
            // SPA：没有扩展名的路径回退到根目录的 index.html
            let index = config.root.join("index.html");
            if config.spa && relative.extension().is_none() && index.is_file() {
                return serve_file(&index, range);
            }
            page(404, "Not Found")
        }
    }
}

#[cfg(test)]
#[path = "handler_test.rs"]
mod tests;
