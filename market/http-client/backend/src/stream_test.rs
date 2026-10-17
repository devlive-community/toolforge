use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use serde_json::Value;

use super::*;
use crate::http::{Body, Pair};

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

fn chunk(out: &mut TcpStream, data: &[u8]) -> bool {
    out.write_all(format!("{:x}\r\n", data.len()).as_bytes())
        .is_ok()
        && out.write_all(data).is_ok()
        && out.write_all(b"\r\n").is_ok()
        && out.flush().is_ok()
}

fn pause(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

/// /sse 分块发送事件（含跨块的 UTF-8 与未结束的尾巴）；/headers 把请求头作为事件发回；
/// /many 连续发送 6000 个事件；/raw 普通文本分块；/hold 发一个事件后保持连接
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || handle(stream));
        }
    });
    format!("http://{addr}")
}

fn handle(mut out: TcpStream) {
    let mut reader = BufReader::new(out.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap_or_default();
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        headers.push(header.trim().to_ascii_lowercase());
    }
    let content_type = if path == "/raw" {
        "text/plain; charset=utf-8"
    } else {
        "text/event-stream; charset=utf-8"
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    if out.write_all(head.as_bytes()).is_err() {
        return;
    }
    let text = "世界".as_bytes();
    match path.as_str() {
        "/sse" => {
            chunk(
                &mut out,
                b"retry: 1500\n: heartbeat\n\nid: 1\nevent: greet\ndata: hel",
            );
            pause(50);
            chunk(&mut out, b"lo\ndata: ");
            chunk(&mut out, &text[..2]);
            pause(50);
            chunk(&mut out, &text[2..]);
            chunk(&mut out, b"\n\ndata: second\n\ndata: unterminated");
        }
        "/headers" => {
            let wanted: Vec<_> = headers
                .iter()
                .filter(|h| {
                    h.starts_with("accept:")
                        || h.starts_with("cache-control:")
                        || h.starts_with("last-event-id:")
                })
                .cloned()
                .collect();
            chunk(
                &mut out,
                format!("data: {}\n\n", wanted.join("|")).as_bytes(),
            );
        }
        "/many" => {
            let mut body = String::new();
            for i in 0..6000 {
                body.push_str(&format!("id: {i}\ndata: {i}\n\n"));
            }
            chunk(&mut out, body.as_bytes());
        }
        "/raw" => {
            chunk(&mut out, b"plain ");
            chunk(&mut out, &text[..1]);
            pause(50);
            chunk(&mut out, &text[1..]);
        }
        "/hold" => {
            chunk(&mut out, b"data: first\n\n");
            for _ in 0..100 {
                pause(50);
                if !chunk(&mut out, b": ping\n") {
                    return;
                }
            }
        }
        _ => {}
    }
    let _ = out.write_all(b"0\r\n\r\n");
}

fn request(url: String) -> Request {
    Request {
        method: "GET".into(),
        url,
        params: vec![],
        headers: vec![],
        body: Body::default(),
        timeout_ms: 30_000,
        follow_redirects: true,
        use_proxy: true,
        stream: true,
    }
}

/// 打开后一直轮询到结束，收集全部事件
fn drain(streams: &Streams, url: String) -> (Opened, Vec<StreamItem>, Batch) {
    let mut opened = streams.open(&request(url), None, false, &Ctx).unwrap();
    let mut events = std::mem::take(&mut opened.batch.events);
    let mut last = streams.poll(opened.session, POLL).unwrap();
    loop {
        events.extend(std::mem::take(&mut last.events));
        if last.done.is_some() {
            return (opened, events, last);
        }
        last = streams.poll(opened.session, POLL).unwrap();
    }
}

#[test]
fn parses_server_sent_events_incrementally() {
    let streams = Streams::default();
    let (opened, events, last) = drain(&streams, format!("{}/sse", serve()));
    assert_eq!((opened.status, opened.mode), (200, Mode::Sse));
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event.as_deref(), Some("greet"));
    assert_eq!(events[0].id.as_deref(), Some("1"));
    assert_eq!(events[0].data, "hello\n世界");
    assert_eq!(events[0].retry, Some(1500));
    // id 延续，event 重置
    assert_eq!(
        (events[1].id.as_deref(), events[1].event.as_deref()),
        (Some("1"), None)
    );
    assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), [1, 2]);
    assert_eq!((last.total, last.comments, last.retry), (2, 1, Some(1500)));
    assert_eq!(last.last_id.as_deref(), Some("1"));
    let done = last.done.unwrap();
    assert_eq!(done.reason, EndReason::Ended);
    assert!(done.discarded);
}

#[test]
fn adds_event_stream_headers_without_overriding() {
    let base = serve();
    let streams = Streams::default();
    let (_, events, _) = drain(&streams, format!("{base}/headers"));
    assert_eq!(
        events[0].data,
        "accept: text/event-stream|cache-control: no-cache"
    );
    let mut custom = request(format!("{base}/headers"));
    custom.headers.push(Pair {
        key: "Accept".into(),
        value: "*/*".into(),
        enabled: true,
    });
    let opened = streams.open(&custom, Some("42"), false, &Ctx).unwrap();
    let mut events = opened.batch.events;
    if events.is_empty() {
        events = streams.poll(opened.session, POLL).unwrap().events;
    }
    assert_eq!(
        events[0].data,
        "accept: */*|cache-control: no-cache|last-event-id: 42"
    );
}

#[test]
fn caps_retained_events_and_batches() {
    let streams = Streams::default();
    let (opened, events, last) = drain(&streams, format!("{}/many", serve()));
    assert!(opened.batch.events.len() <= BATCH);
    assert_eq!(last.total, 6000);
    assert_eq!(last.dropped, 1000);
    // 收到的事件不重复、按序递增；被丢弃的最早事件不会再交出
    assert!(events.windows(2).all(|w| w[0].seq < w[1].seq));
    assert_eq!(events.last().unwrap().data, "5999");
    let exported: Value =
        serde_json::from_str(&streams.export(opened.session, ExportFormat::Json).unwrap()).unwrap();
    assert_eq!(exported["events"].as_array().unwrap().len(), MAX_EVENTS);
    assert_eq!(exported["events"][0]["data"], "1000");
    assert_eq!(exported["dropped"], 1000);
}

#[test]
fn shows_other_responses_as_chunks() {
    let streams = Streams::default();
    let (opened, events, last) = drain(&streams, format!("{}/raw", serve()));
    assert_eq!(opened.mode, Mode::Raw);
    assert!(events.iter().all(|e| e.kind == Kind::Chunk));
    assert_eq!(
        events.iter().map(|e| e.data.as_str()).collect::<String>(),
        "plain 世界"
    );
    assert!(!last.done.unwrap().discarded);
    assert_eq!(
        streams.export(opened.session, ExportFormat::Sse).unwrap(),
        "plain 世界"
    );
}

#[test]
fn stops_and_closes_sessions() {
    let streams = Streams::default();
    let opened = streams
        .open(&request(format!("{}/hold", serve())), None, false, &Ctx)
        .unwrap();
    let mut seen = opened.batch.events.len();
    while seen == 0 {
        seen += streams.poll(opened.session, POLL).unwrap().events.len();
    }
    let started = Instant::now();
    let pending = streams
        .poll(opened.session, Duration::from_millis(300))
        .unwrap();
    // 只有心跳时轮询按预算返回，不会一直挂起
    assert!(pending.events.is_empty() && pending.done.is_none());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(pending.comments > 0);
    let stopped = streams.stop(opened.session).unwrap();
    assert_eq!(stopped.done.unwrap().reason, EndReason::Stopped);
    // 停止后仍可导出；关闭后会话失效
    let sse = streams.export(opened.session, ExportFormat::Sse).unwrap();
    assert_eq!(sse, "data: first\n\n");
    streams.close(opened.session);
    assert_eq!(
        streams.poll(opened.session, POLL).unwrap_err().code,
        "http.stream_expired"
    );
}

#[test]
fn reports_connection_errors() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let err = Streams::default()
        .open(
            &request(format!("http://127.0.0.1:{port}/")),
            None,
            false,
            &Ctx,
        )
        .err()
        .unwrap();
    assert_eq!(err.code, "http.connect_failed");
}

#[test]
fn exports_events_as_text_stream() {
    let item = StreamItem {
        seq: 1,
        at_ms: 0,
        kind: Kind::Event,
        event: Some("update".into()),
        id: Some("9".into()),
        data: "a\nb".into(),
        formatted: None,
        retry: Some(100),
        truncated: false,
    };
    assert_eq!(
        to_sse(&item, Mode::Sse),
        "event: update\nid: 9\nretry: 100\ndata: a\ndata: b\n\n"
    );
    assert!(is_event_stream(Some("Text/Event-Stream; charset=utf-8")));
    assert!(!is_event_stream(Some("text/plain")));
    assert!(!is_event_stream(None));
    assert_eq!(
        format_json("{\"a\":[1]}").as_deref(),
        Some("{\n  \"a\": [\n    1\n  ]\n}")
    );
    assert_eq!(format_json("[DONE]"), None);
    assert_eq!(format_json("42"), None);
    assert_eq!(
        format_json(&format!("[{}]", "1,".repeat(40_000) + "1")),
        None
    );
}
