use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::channel::Channel;
use hyper::service::service_fn;

use super::*;

type Body = Channel<Bytes, Infallible>;

/// /stream 分三块发送并带 trailers；/slow 发送一块后不再结束；/echo 回显方法、头与正文
async fn handle(request: http::Request<Incoming>) -> Result<http::Response<Body>, Infallible> {
    let path = request.uri().path().to_owned();
    if path == "/hold" {
        // 迟迟不发送响应头：用于测试等待期间取消
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
    let (mut tx, body) = Channel::new(8);
    let header = request
        .headers()
        .get("x-test")
        .map(|v| v.to_str().unwrap_or_default().to_owned())
        .unwrap_or_default();
    let method = request.method().to_string();
    let incoming = request
        .into_body()
        .collect()
        .await
        .map(|b| b.to_bytes())
        .unwrap_or_default();
    tokio::spawn(async move {
        match path.as_str() {
            "/stream" => {
                for part in ["one", "two", "three"] {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    if tx.send_data(Bytes::from(part)).await.is_err() {
                        return;
                    }
                }
                let mut trailers = http::HeaderMap::new();
                trailers.insert("grpc-status", "0".parse().unwrap());
                let _ = tx.send_trailers(trailers).await;
            }
            "/slow" => {
                let _ = tx.send_data(Bytes::from("first")).await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            _ => {
                let text = format!("{method} {header} {}", String::from_utf8_lossy(&incoming));
                let _ = tx.send_data(Bytes::from(text)).await;
            }
        }
    });
    Ok(http::Response::builder()
        .header("content-type", "text/plain")
        .body(body)
        .unwrap())
}

/// 启动测试服务：h2 为真时只接受 HTTP/2 先验知识（h2c），否则 HTTP/1.1
fn serve(h2: bool) -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    runtime().spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let io = TokioIo::new(tcp);
                let service = service_fn(handle);
                if h2 {
                    let _ = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
                        .serve_connection(io, service)
                        .await;
                } else {
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, service)
                        .await;
                }
            });
        }
    });
    addr
}

fn read_all(stream: &mut HttpStream) -> (Vec<String>, Vec<(String, String)>) {
    let mut chunks = Vec::new();
    loop {
        match stream.read(Duration::from_secs(5)).unwrap() {
            StreamEvent::Data(data) => chunks.push(String::from_utf8(data).unwrap()),
            StreamEvent::Pending => {}
            StreamEvent::End(trailers) => return (chunks, trailers),
        }
    }
}

#[test]
fn streams_http2_chunks_and_trailers() {
    let addr = serve(true);
    let mut request = StreamRequest::new("POST", &format!("http://{addr}/stream"));
    request.http2_prior_knowledge = true;
    let (mut stream, head) = HttpStream::open(&request, &|| false).unwrap();
    assert_eq!((head.status, head.version.as_str()), (200, "HTTP/2"));
    assert!(
        head.headers
            .iter()
            .any(|(k, v)| k == "content-type" && v == "text/plain")
    );
    let (chunks, trailers) = read_all(&mut stream);
    assert_eq!(chunks.concat(), "onetwothree");
    assert_eq!(trailers, [("grpc-status".to_owned(), "0".to_owned())]);
    assert_eq!(
        stream.read(Duration::from_millis(10)).unwrap_err().code,
        "net.stream_closed"
    );
}

#[test]
fn sends_headers_and_body_over_both_protocols() {
    for h2 in [true, false] {
        let addr = serve(h2);
        let mut request = StreamRequest::new("PUT", &format!("http://{addr}/echo?x=1"));
        request.http2_prior_knowledge = h2;
        request.headers.push(("x-test".into(), "hi".into()));
        request.body = Some(b"payload".to_vec());
        let (mut stream, head) = HttpStream::open(&request, &|| false).unwrap();
        assert_eq!(head.version, if h2 { "HTTP/2" } else { "HTTP/1.1" });
        let (chunks, trailers) = read_all(&mut stream);
        assert_eq!(chunks.concat(), "PUT hi payload");
        assert!(trailers.is_empty());
    }
}

#[test]
fn reports_pending_reads_and_deadlines() {
    let addr = serve(true);
    let mut request = StreamRequest::new("GET", &format!("http://{addr}/slow"));
    request.http2_prior_knowledge = true;
    request.timeout_ms = 1500;
    let (mut stream, _) = HttpStream::open(&request, &|| false).unwrap();
    assert_eq!(
        stream.read(Duration::from_secs(5)).unwrap(),
        StreamEvent::Data(b"first".to_vec())
    );
    assert_eq!(
        stream.read(Duration::from_millis(50)).unwrap(),
        StreamEvent::Pending
    );
    // 截止时间到了，流以超时结束
    let err = loop {
        match stream.read(Duration::from_millis(500)) {
            Ok(StreamEvent::Pending) => continue,
            other => break other.unwrap_err(),
        }
    };
    assert_eq!(err.code, "net.timeout");
}

#[test]
fn reports_connection_and_request_errors() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let err = HttpStream::open(
        &StreamRequest::new("GET", &format!("http://127.0.0.1:{port}/")),
        &|| false,
    )
    .err()
    .unwrap();
    assert_eq!(err.code, "net.connect");
    let err = HttpStream::open(&StreamRequest::new("GET", "ws://x/"), &|| false)
        .err()
        .unwrap();
    assert_eq!(err.code, "net.unsupported_scheme");
    let mut bad = StreamRequest::new("GET", "http://127.0.0.1:1/");
    bad.headers.push(("bad header".into(), "x".into()));
    // 连接失败先于请求头校验也可以，只要不 panic
    assert!(HttpStream::open(&bad, &|| false).is_err());
}

#[test]
fn cancels_while_waiting_for_headers() {
    let addr = serve(true);
    let mut request = StreamRequest::new("GET", &format!("http://{addr}/hold"));
    request.http2_prior_knowledge = true;
    let started = std::time::Instant::now();
    let calls = std::cell::Cell::new(0);
    let err = HttpStream::open(&request, &|| {
        calls.set(calls.get() + 1);
        calls.get() > 2
    })
    .err()
    .unwrap();
    assert_eq!(err.code, "task.cancelled");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn dropping_a_stream_releases_it() {
    let addr = serve(true);
    let mut request = StreamRequest::new("GET", &format!("http://{addr}/slow"));
    request.http2_prior_knowledge = true;
    let (stream, _) = HttpStream::open(&request, &|| false).unwrap();
    drop(stream);
    // 同一服务仍可继续使用
    let mut request = StreamRequest::new("GET", &format!("http://{addr}/stream"));
    request.http2_prior_knowledge = true;
    let (mut stream, _) = HttpStream::open(&request, &|| false).unwrap();
    assert_eq!(read_all(&mut stream).0.concat(), "onetwothree");
}

/// 需要联网：验证 TLS（系统证书校验）与 ALPN 协商。
/// `cargo test -p tf-net tls_with_system_roots -- --ignored`
#[test]
#[ignore]
fn tls_with_system_roots() {
    let (mut stream, head) = HttpStream::open(
        &StreamRequest::new("GET", "https://www.example.com/"),
        &|| false,
    )
    .unwrap();
    assert_eq!(head.status, 200);
    println!("negotiated {}", head.version);
    assert!(!read_all(&mut stream).0.concat().is_empty());
    let err = HttpStream::open(
        &StreamRequest::new("GET", "https://self-signed.badssl.com/"),
        &|| false,
    )
    .err()
    .unwrap();
    assert_eq!(err.code, "net.tls");
}
