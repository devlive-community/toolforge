use std::sync::atomic::AtomicUsize;

use super::*;

/// /echo 回显方法、请求体与 Authorization；/same 302 到同源；/cross 302 到另一个服务；/big 返回 2 KB
fn server(other: Option<String>) -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let base = format!(
        "http://127.0.0.1:{}",
        server.server_addr().to_ip().unwrap().port()
    );
    let own = base.clone();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            let header = |name: &str| {
                request
                    .headers()
                    .iter()
                    .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
                    .map(|h| h.value.to_string())
                    .unwrap_or_default()
            };
            let echo = format!(
                "{} body=[{body}] auth=[{}] ua=[{}]",
                request.method(),
                header("Authorization"),
                header("User-Agent")
            );
            let path = request
                .url()
                .split('?')
                .next()
                .unwrap_or_default()
                .to_owned();
            let redirect = |to: String| {
                tiny_http::Response::from_string("")
                    .with_status_code(302)
                    .with_header(tiny_http::Header::from_bytes("Location", to).unwrap())
            };
            let _ = match path.as_str() {
                "/same" => request.respond(redirect(format!("{own}/echo"))),
                "/cross" => request.respond(redirect(format!(
                    "{}/echo",
                    other.clone().unwrap_or_default()
                ))),
                "/loop" => request.respond(redirect(format!("{own}/loop"))),
                "/big" => request.respond(tiny_http::Response::from_data(vec![b'x'; 2048])),
                _ => request.respond(tiny_http::Response::from_string(echo).with_status_code(201)),
            };
        }
    });
    base
}

fn get(url: &str) -> HttpRequest {
    HttpRequest::new("GET", url)
}

fn run(request: &HttpRequest) -> PluginResult<HttpResponse> {
    send(request, &|| false, &|_, _| {})
}

fn text(response: &HttpResponse) -> String {
    String::from_utf8_lossy(&response.body).into_owned()
}

#[test]
fn sends_requests_and_reads_responses() {
    let base = server(None);
    let mut request = HttpRequest::new("POST", &format!("{base}/echo"));
    request.headers.push(("X-Test".into(), "1".into()));
    request.body = Some(b"hello".to_vec());
    let response = run(&request).unwrap();
    assert_eq!(
        (response.status, response.reason.as_str()),
        (201, "Created")
    );
    assert_eq!(response.version, "HTTP/1.1");
    assert!(text(&response).starts_with("POST body=[hello] auth=[] ua=[ToolForge/"));
    assert!(response.redirects.is_empty());
    assert!(response.headers.iter().any(|(k, _)| k == "content-length"));
    // 插件自己指定的 User-Agent 不会被覆盖
    let mut request = get(&format!("{base}/echo"));
    request.headers.push(("User-Agent".into(), "custom".into()));
    assert!(text(&run(&request).unwrap()).contains("ua=[custom]"));
}

#[test]
fn drops_credentials_on_cross_origin_redirects() {
    let other = server(None);
    let base = server(Some(other.clone()));
    let mut request = get(&format!("{base}/same"));
    request
        .headers
        .push(("Authorization".into(), "Bearer t".into()));
    let response = run(&request).unwrap();
    assert!(text(&response).contains("auth=[Bearer t]"));
    assert_eq!(response.redirects, [format!("{base}/echo")]);
    request.url = format!("{base}/cross");
    let response = run(&request).unwrap();
    assert!(text(&response).contains("auth=[]"));
    assert_eq!(response.url, format!("{other}/echo"));
    // 不跟随时返回 302 本身
    request.follow_redirects = false;
    assert_eq!(run(&request).unwrap().status, 302);
    request.follow_redirects = true;
    request.url = format!("{base}/loop");
    request.max_redirects = 3;
    assert_eq!(run(&request).unwrap_err().code, "net.redirects");
}

#[test]
fn enforces_limits_and_cancellation() {
    let base = server(None);
    let mut request = get(&format!("{base}/big"));
    request.max_body = 1024;
    let err = run(&request).unwrap_err();
    assert_eq!(
        (err.code.as_str(), &err.params["limit"]),
        ("net.too_large", &1024.into())
    );
    request.max_body = 4096;
    let reported = AtomicUsize::new(0);
    let response = send(&request, &|| false, &|done, total| {
        assert_eq!(total, 2048);
        reported.store(done as usize, Ordering::SeqCst);
    })
    .unwrap();
    assert_eq!(
        (response.body.len(), reported.load(Ordering::SeqCst)),
        (2048, 2048)
    );
    // 读取响应体前检查取消
    let err = send(&request, &|| true, &|_, _| {}).unwrap_err();
    assert_eq!(err.code, "task.cancelled");
    assert_eq!(
        run(&get("ftp://x/")).unwrap_err().code,
        "net.unsupported_scheme"
    );
    assert_eq!(
        run(&get("not a url")).unwrap_err().code,
        "net.invalid_request"
    );
    let mut bad = get(&format!("{base}/echo"));
    bad.headers.push(("bad header".into(), "x".into()));
    assert_eq!(run(&bad).unwrap_err().code, "net.invalid_request");
    // 关闭的端口
    let closed = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let mut refused = get(&format!("http://127.0.0.1:{port}/"));
    refused.timeout_ms = 3000;
    assert!(matches!(
        run(&refused).unwrap_err().code.as_str(),
        "net.connect" | "net.timeout"
    ));
}

fn hit(port: u16, target: &str) -> Result<u16, String> {
    ensure_crypto_provider();
    reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://127.0.0.1:{port}{target}"))
        .send()
        .map(|r| r.status().as_u16())
        .map_err(|e| e.to_string())
}

#[test]
fn callbacks_accept_one_matching_request() {
    let callback = CallbackServer::listen(0, "/cb", Duration::from_secs(30)).unwrap();
    let port = callback.port();
    assert_ne!(port, 0);
    assert_eq!(callback.poll().unwrap(), None);
    assert_eq!(hit(port, "/other?code=1"), Ok(404));
    assert_eq!(callback.poll().unwrap(), None);
    assert_eq!(hit(port, "/cb?code=abc&state=xyz"), Ok(200));
    assert_eq!(
        callback.poll().unwrap().as_deref(),
        Some("code=abc&state=xyz")
    );
    // 只接受一次：之后连不上
    std::thread::sleep(Duration::from_millis(200));
    assert!(hit(port, "/cb?code=again").is_err());
    drop(callback);
    // 端口已释放，可以再次使用
    let again = CallbackServer::listen(port, "/cb", Duration::from_secs(5)).unwrap();
    // 端口被占用
    let err = CallbackServer::listen(port, "/cb", Duration::from_secs(5))
        .err()
        .unwrap();
    assert_eq!(err.code, "net.port_in_use");
    drop(again);
    assert!(hit(port, "/cb").is_err());
}

#[test]
fn callbacks_time_out_and_validate_paths() {
    let callback = CallbackServer::listen(0, "/cb", Duration::from_secs(1)).unwrap();
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(callback.poll().unwrap_err().code, "net.callback_timeout");
    assert!(hit(callback.port(), "/cb").is_err());
    for path in ["cb", "/a b", "/a?b"] {
        assert_eq!(
            CallbackServer::listen(0, path, Duration::from_secs(1))
                .err()
                .unwrap()
                .code,
            "net.invalid_path"
        );
    }
}

#[test]
fn writes_files() {
    let path = std::env::temp_dir().join(format!("tf-net-{}.bin", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    write_file(&path, &[1, 2, 3]).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), [1, 2, 3]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        write_file("/nonexistent/dir/x", b"x").unwrap_err().code,
        "fs.io"
    );
}
