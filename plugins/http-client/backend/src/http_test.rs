use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use super::*;

struct Ctx;

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}

/// /echo 回显请求；/redirect 302 到 /echo；/bin 返回二进制；/big 返回超长文本；/slow 延迟 3 秒
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap_or_default();
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or_default().to_owned();
                let target = parts.next().unwrap_or("/").to_owned();
                let mut headers = serde_json::Map::new();
                let mut length = 0;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some((k, v)) = h.trim_end().split_once(": ") {
                        if k.eq_ignore_ascii_case("content-length") {
                            length = v.parse().unwrap_or(0);
                        }
                        headers.insert(k.to_ascii_lowercase(), Value::String(v.to_owned()));
                    }
                }
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).ok();
                let mut stream = stream;
                let path = target.split('?').next().unwrap_or_default();
                let (status, content_type, payload, extra): (&str, &str, Vec<u8>, String) =
                    match path {
                        "/redirect" => (
                            "302 Found",
                            "text/plain",
                            Vec::new(),
                            "Location: /echo?from=redirect\r\n".into(),
                        ),
                        "/bin" => (
                            "200 OK",
                            "application/octet-stream",
                            vec![0, 159, 146, 150],
                            String::new(),
                        ),
                        "/big" => (
                            "200 OK",
                            "text/plain",
                            vec![b'a'; PREVIEW + 10],
                            String::new(),
                        ),
                        "/slow" => {
                            thread::sleep(Duration::from_secs(3));
                            ("200 OK", "text/plain", b"late".to_vec(), String::new())
                        }
                        "/echo" => {
                            let echo = json!({
                                "method": method,
                                "target": target,
                                "headers": headers,
                                "body": String::from_utf8_lossy(&body),
                            });
                            (
                                "201 Created",
                                "application/json",
                                serde_json::to_vec(&echo).unwrap(),
                                String::new(),
                            )
                        }
                        _ => (
                            "404 Not Found",
                            "text/plain",
                            b"nope".to_vec(),
                            String::new(),
                        ),
                    };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&payload);
            });
        }
    });
    format!("http://{addr}")
}

fn request(method: &str, url: String) -> Request {
    Request {
        method: method.into(),
        url,
        params: vec![],
        headers: vec![],
        body: Body::default(),
        timeout_ms: 30_000,
        follow_redirects: true,
        use_proxy: true,
    }
}

fn pair(key: &str, value: &str) -> Pair {
    Pair {
        key: key.into(),
        value: value.into(),
        enabled: true,
    }
}

#[test]
fn sends_json_with_params_and_headers() {
    let base = serve();
    let mut req = request("post", format!("{base}/echo?a=1"));
    req.params = vec![
        pair("q", "hello world"),
        Pair {
            enabled: false,
            ..pair("skip", "x")
        },
    ];
    req.headers = vec![pair("X-Token", "abc")];
    req.body = Body {
        kind: BodyKind::Json,
        text: r#"{"name":"toolforge"}"#.into(),
        form: vec![],
    };
    let bodies = Bodies::default();
    let reply = send(req, &bodies, &Ctx).unwrap();
    assert_eq!((reply.status, reply.status_text.as_str()), (201, "Created"));
    assert_eq!(reply.kind, ContentKind::Json);
    let echo: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(echo["method"], "POST");
    assert_eq!(echo["target"], "/echo?a=1&q=hello+world");
    assert_eq!(echo["headers"]["x-token"], "abc");
    assert_eq!(echo["headers"]["content-type"], "application/json");
    assert_eq!(echo["body"], r#"{"name":"toolforge"}"#);
    // JSON 已格式化
    assert!(reply.body.contains("\n  \""));
    assert!(reply.headers.iter().any(|(k, _)| k == "content-type"));
}

#[test]
fn form_body_and_explicit_content_type() {
    let base = serve();
    let mut req = request("PUT", format!("{base}/echo"));
    req.body = Body {
        kind: BodyKind::Form,
        text: String::new(),
        form: vec![pair("a", "1 2"), pair("b", "&")],
    };
    let reply = send(req, &Bodies::default(), &Ctx).unwrap();
    let echo: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(echo["body"], "a=1+2&b=%26");
    assert_eq!(
        echo["headers"]["content-type"],
        "application/x-www-form-urlencoded"
    );

    let mut req = request("POST", format!("{base}/echo"));
    req.headers = vec![pair("content-type", "text/csv")];
    req.body = Body {
        kind: BodyKind::Text,
        text: "x,y".into(),
        form: vec![],
    };
    let echo: Value =
        serde_json::from_str(&send(req, &Bodies::default(), &Ctx).unwrap().body).unwrap();
    assert_eq!(echo["headers"]["content-type"], "text/csv");
}

#[test]
fn follows_or_stops_at_redirects() {
    let base = serve();
    let reply = send(
        request("GET", format!("{base}/redirect")),
        &Bodies::default(),
        &Ctx,
    )
    .unwrap();
    assert_eq!(reply.status, 201);
    assert_eq!(reply.redirects.len(), 1);
    assert!(reply.url.ends_with("/echo?from=redirect"));

    let mut req = request("GET", format!("{base}/redirect"));
    req.follow_redirects = false;
    let reply = send(req, &Bodies::default(), &Ctx).unwrap();
    assert_eq!(reply.status, 302);
    assert!(reply.redirects.is_empty());
}

#[test]
fn binary_and_truncated_bodies_can_be_saved() {
    let base = serve();
    let bodies = Bodies::default();
    let reply = send(request("GET", format!("{base}/bin")), &bodies, &Ctx).unwrap();
    assert_eq!(
        (reply.kind, reply.size, reply.body.as_str()),
        (ContentKind::Binary, 4, "")
    );
    let path = std::env::temp_dir().join(format!("tfp-http-{}.bin", std::process::id()));
    bodies
        .save(SaveArgs {
            id: reply.id,
            path: path.to_string_lossy().into_owned(),
        })
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), vec![0, 159, 146, 150]);

    let big = send(request("GET", format!("{base}/big")), &bodies, &Ctx).unwrap();
    assert!(big.truncated);
    assert_eq!(big.body.len(), PREVIEW);
    assert_eq!(big.size, PREVIEW as u64 + 10);
    // 旧响应已被新响应替换
    assert_eq!(
        bodies
            .save(SaveArgs {
                id: reply.id,
                path: path.to_string_lossy().into_owned()
            })
            .unwrap_err()
            .code,
        "http.response_expired"
    );
}

#[test]
fn reports_errors() {
    let base = serve();
    let mut slow = request("GET", format!("{base}/slow"));
    slow.timeout_ms = 1000;
    assert_eq!(
        send(slow, &Bodies::default(), &Ctx).unwrap_err().code,
        "http.timeout"
    );
    assert_eq!(
        send(
            request("GET", "http://127.0.0.1:1/".into()),
            &Bodies::default(),
            &Ctx
        )
        .unwrap_err()
        .code,
        "http.connect_failed"
    );
    let mut bad_json = request("POST", format!("{base}/echo"));
    bad_json.body = Body {
        kind: BodyKind::Json,
        text: "{oops".into(),
        form: vec![],
    };
    assert_eq!(
        send(bad_json, &Bodies::default(), &Ctx).unwrap_err().code,
        "http.invalid_json_body"
    );
    assert_eq!(
        send(request("GE T", base.clone()), &Bodies::default(), &Ctx)
            .unwrap_err()
            .code,
        "http.invalid_method"
    );
    let mut bad_header = request("GET", base);
    bad_header.headers = vec![pair("bad header", "x")];
    assert_eq!(
        send(bad_header, &Bodies::default(), &Ctx).unwrap_err().code,
        "http.invalid_header"
    );
}

#[test]
fn loopback_bypasses_proxy() {
    assert!(is_loopback(&normalize_url("localhost:8080").unwrap()));
    assert!(is_loopback(&normalize_url("http://127.0.0.1/").unwrap()));
    assert!(is_loopback(&normalize_url("http://[::1]:3000").unwrap()));
    assert!(!is_loopback(&normalize_url("example.com").unwrap()));
}

#[test]
fn normalizes_urls() {
    assert_eq!(
        normalize_url("example.com/a").unwrap().as_str(),
        "https://example.com/a"
    );
    assert_eq!(
        normalize_url("localhost:3000/api").unwrap().as_str(),
        "http://localhost:3000/api"
    );
    assert_eq!(normalize_url("192.168.1.2").unwrap().scheme(), "http");
    assert_eq!(normalize_url("  ").unwrap_err().code, "http.empty_url");
    assert_eq!(
        normalize_url("ftp://x").unwrap_err().code,
        "http.unsupported_scheme"
    );
}

#[test]
fn renders_json_and_text() {
    let (kind, text, truncated) = render(&ContentKind::Json, br#"{"a":[1,2]}"#);
    assert_eq!((kind, truncated), (ContentKind::Json, false));
    assert_eq!(text, "{\n  \"a\": [\n    1,\n    2\n  ]\n}");
    // 声明为 JSON 但无法解析时按文本展示
    let (kind, text, _) = render(&ContentKind::Json, b"not json");
    assert_eq!((kind, text.as_str()), (ContentKind::Text, "not json"));
    assert_eq!(
        classify(Some("text/html; charset=utf-8"), b"<p>"),
        ContentKind::Text
    );
    assert_eq!(classify(None, b"plain"), ContentKind::Text);
    assert_eq!(
        classify(Some("image/png"), &[0x89, 0x50]),
        ContentKind::Binary
    );
}

#[test]
fn builds_curl_commands() {
    let mut req = request("POST", "https://api.example.com/users".into());
    req.params = vec![pair("page", "2")];
    req.headers = vec![pair("Authorization", "Bearer it's")];
    req.body = Body {
        kind: BodyKind::Json,
        text: r#"{"a":1}"#.into(),
        form: vec![],
    };
    let curl = to_curl(req).unwrap();
    assert_eq!(
        curl,
        "curl \\\n  -X POST \\\n  'https://api.example.com/users?page=2' \\\n  -H 'Authorization: Bearer it'\\''s' \\\n  -H 'Content-Type: application/json' \\\n  --data-raw '{\"a\":1}' \\\n  -L"
    );
    let mut get = request("GET", "example.com".into());
    get.follow_redirects = false;
    assert_eq!(to_curl(get).unwrap(), "curl \\\n  'https://example.com/'");
}
