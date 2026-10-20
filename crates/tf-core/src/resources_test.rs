use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use serde_json::Value;

use super::*;

/// 测试用 HTTP 服务：/ok 支持 Range，/norange 忽略 Range，/short 发一半后断开，其余 404
fn serve(data: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let data = data.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap_or_default();
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut range = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        range = v.trim().trim_end_matches('-').parse::<usize>().ok();
                    }
                }
                let mut stream = stream;
                let (status, body): (&str, &[u8]) = match (path.as_str(), range) {
                    ("/ok", Some(from)) => ("206 Partial Content", &data[from..]),
                    ("/ok" | "/norange", _) => ("200 OK", &data),
                    ("/short", _) => ("200 OK", &data),
                    _ => ("404 Not Found", b""),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
                    body.len(),
                    if let ("/ok", Some(from)) = (path.as_str(), range) {
                        format!(
                            "Content-Range: bytes {}-{}/{}\r\n",
                            from,
                            data.len() - 1,
                            data.len()
                        )
                    } else {
                        String::new()
                    }
                );
                let _ = stream.write_all(head.as_bytes());
                let send = if path == "/short" {
                    &body[..body.len() / 2]
                } else {
                    body
                };
                let _ = stream.write_all(send);
            });
        }
    });
    format!("http://{addr}")
}

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<String>>,
    cancel_after: Option<u64>,
    cancelled: AtomicBool,
    last: Mutex<u64>,
    cancel_on_retry: bool,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
        if self.cancel_on_retry && code == "resource.retry" {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, done: u64, _: u64) {
        *self.last.lock().unwrap() = done;
        if self.cancel_after.is_some_and(|limit| done >= limit) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        unreachable!()
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        unreachable!()
    }
}

impl Ctx {
    fn has(&self, code: &str) -> bool {
        self.logs.lock().unwrap().iter().any(|c| c == code)
    }
}

fn payload() -> Vec<u8> {
    (0..2_000_000u32).map(|i| (i % 251) as u8).collect()
}

fn spec(urls: Vec<String>, data: &[u8]) -> ResourceSpec {
    ResourceSpec {
        id: "model".into(),
        urls,
        sha256: to_hex(&Sha256::digest(data)),
        size: data.len() as u64,
        license: Some("MIT".into()),
    }
}

fn store(name: &str) -> Resources {
    let root = std::env::temp_dir().join(format!("tf-resources-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    Resources::new(root)
}

const PLUGIN: &str = "org.devlive.toolforge.test";

#[test]
fn downloads_verifies_and_reports_status() {
    let data = payload();
    let base = serve(data.clone());
    let resources = store("ok");
    let spec = spec(vec![format!("{base}/ok")], &data);
    assert!(!resources.status(PLUGIN, std::slice::from_ref(&spec))[0].installed);

    let ctx = Ctx::default();
    let path = resources.download(PLUGIN, &spec, &ctx).unwrap();
    assert_eq!(fs::read(&path).unwrap(), data);
    assert_eq!(resources.path(PLUGIN, "model"), Some(path));
    assert_eq!(*ctx.last.lock().unwrap(), data.len() as u64);
    assert!(ctx.has("resource.verified") && ctx.has("resource.installed"));

    let status = &resources.status(PLUGIN, std::slice::from_ref(&spec))[0];
    assert!(status.installed && status.installed_at.is_some());
    assert_eq!(status.partial, 0);

    // 再次下载直接复用
    let again = Ctx::default();
    resources.download(PLUGIN, &spec, &again).unwrap();
    assert!(again.has("resource.already_installed"));

    resources.remove(PLUGIN, "model").unwrap();
    assert!(resources.path(PLUGIN, "model").is_none());
    resources.remove(PLUGIN, "model").unwrap();
}

#[test]
fn falls_back_to_mirrors() {
    let data = payload();
    let base = serve(data.clone());
    let resources = store("mirror");
    let spec = spec(vec![format!("{base}/missing"), format!("{base}/ok")], &data);
    let ctx = Ctx::default();
    resources.download(PLUGIN, &spec, &ctx).unwrap();
    assert!(ctx.has("resource.source_failed"));
}

#[test]
fn rejects_checksum_mismatch() {
    let data = payload();
    let base = serve(data.clone());
    let resources = store("checksum");
    let mut spec = spec(vec![format!("{base}/ok")], &data);
    spec.sha256 = "00".repeat(32);
    let err = resources
        .download(PLUGIN, &spec, &Ctx::default())
        .unwrap_err();
    assert_eq!(err.code, "resource.checksum_mismatch");
    assert!(resources.path(PLUGIN, "model").is_none());
    assert_eq!(resources.status(PLUGIN, &[spec])[0].partial, 0);
}

#[test]
fn cancel_then_resume() {
    let data = payload();
    let base = serve(data.clone());
    let resources = store("resume");
    let spec = spec(vec![format!("{base}/ok")], &data);

    let ctx = Ctx {
        cancel_after: Some(500_000),
        ..Ctx::default()
    };
    let err = resources.download(PLUGIN, &spec, &ctx).unwrap_err();
    assert_eq!(err.code, "task.cancelled");
    let partial = resources.status(PLUGIN, std::slice::from_ref(&spec))[0].partial;
    assert!(partial > 0 && partial < data.len() as u64, "{partial}");

    let ctx = Ctx::default();
    let path = resources.download(PLUGIN, &spec, &ctx).unwrap();
    assert!(ctx.has("resource.resumed"));
    assert_eq!(fs::read(path).unwrap(), data);
}

#[test]
fn restarts_when_server_ignores_range() {
    let data = payload();
    let base = serve(data.clone());
    let resources = store("norange");
    let spec = spec(vec![format!("{base}/norange")], &data);
    let (_, _, part) = resources.paths(PLUGIN, "model").unwrap();
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, &data[..1000]).unwrap();

    let ctx = Ctx::default();
    let path = resources.download(PLUGIN, &spec, &ctx).unwrap();
    assert!(!ctx.has("resource.resumed"));
    assert_eq!(fs::read(path).unwrap(), data);
}

#[test]
fn interrupted_transfer_keeps_partial_data() {
    let data = payload();
    let base = serve(data.clone());
    let resources = store("short");
    let spec = spec(vec![format!("{base}/short")], &data);
    let err = resources
        .download(PLUGIN, &spec, &Ctx::default())
        .unwrap_err();
    assert!(
        ["resource.size_mismatch", "resource.network"].contains(&err.code.as_str()),
        "{}",
        err.code
    );
    assert!(resources.status(PLUGIN, &[spec])[0].partial > 0);
}

#[test]
fn rejects_unsafe_ids() {
    let resources = store("ids");
    let mut bad = spec(vec!["http://x".into()], b"x");
    bad.id = "../evil".into();
    assert_eq!(
        resources
            .download(PLUGIN, &bad, &Ctx::default())
            .unwrap_err()
            .code,
        "resource.invalid_id"
    );
    assert!(resources.remove("../x", "model").is_err());
    assert!(resources.path(PLUGIN, "../evil").is_none());
}

#[derive(Clone, Copy)]
enum Reply {
    Good,
    Short,
    BadRange,
    Status(u16),
}

type Requests = std::sync::Arc<Mutex<Vec<Option<usize>>>>;

/// 有界 HTTP 故障夹具，保留每次请求的续传偏移。
fn scripted(data: Vec<u8>, replies: Vec<Reply>) -> (String, Requests, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/model", listener.local_addr().unwrap());
    let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let worker = thread::spawn(move || {
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "missing download request");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut offset = None;
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    offset = value.trim().trim_end_matches('-').parse::<usize>().ok();
                }
            }
            seen.lock().unwrap().push(offset);
            let start = offset.unwrap_or(0).min(data.len());
            let body = &data[start..];
            match reply {
                Reply::Good | Reply::Short => {
                    let status = if offset.is_some() {
                        "206 Partial Content"
                    } else {
                        "200 OK"
                    };
                    let range = if offset.is_some() {
                        format!(
                            "Content-Range: bytes {start}-{}/{}\r\n",
                            data.len() - 1,
                            data.len()
                        )
                    } else {
                        String::new()
                    };
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{range}Connection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(header.as_bytes()).unwrap();
                    let send = if matches!(reply, Reply::Short) {
                        &body[..body.len() / 2]
                    } else {
                        body
                    };
                    let _ = stream.write_all(send);
                }
                Reply::BadRange => {
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        start + 1,
                        data.len() - 1,
                        data.len()
                    );
                    stream.write_all(header.as_bytes()).unwrap();
                }
                Reply::Status(status) => {
                    stream.write_all(format!("HTTP/1.1 {status} Failure\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
                }
            }
        }
    });
    (url, requests, worker)
}

#[test]
fn retries_a_disconnected_download_from_the_preserved_offset() {
    let data = payload();
    let (url, requests, worker) = scripted(data.clone(), vec![Reply::Short, Reply::Good]);
    let resources = store("auto-resume");
    let ctx = Ctx::default();
    let path = resources
        .download(PLUGIN, &spec(vec![url], &data), &ctx)
        .unwrap();
    worker.join().unwrap();
    assert_eq!(fs::read(path).unwrap(), data);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], None);
    assert!(requests[1].is_some_and(|offset| offset > 0 && offset <= data.len() / 2));
    assert!(ctx.has("resource.retry") && ctx.has("resource.resumed"));
}

#[test]
fn invalid_or_rejected_resume_restarts_without_appending_bad_bytes() {
    for (name, first) in [
        ("invalid-range", Reply::BadRange),
        ("range-416", Reply::Status(416)),
    ] {
        let data = payload();
        let (url, requests, worker) = scripted(data.clone(), vec![first, Reply::Good]);
        let resources = store(name);
        let (_, _, part) = resources.paths(PLUGIN, "model").unwrap();
        fs::create_dir_all(part.parent().unwrap()).unwrap();
        fs::write(&part, &data[..1000]).unwrap();
        let path = resources
            .download(PLUGIN, &spec(vec![url], &data), &Ctx::default())
            .unwrap();
        worker.join().unwrap();
        assert_eq!(fs::read(path).unwrap(), data);
        assert_eq!(*requests.lock().unwrap(), vec![Some(1000), None]);
    }
}

#[test]
fn installs_a_complete_partial_file_without_a_network_request() {
    let data = payload();
    let resources = store("complete-part");
    let (_, _, part) = resources.paths(PLUGIN, "model").unwrap();
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, &data).unwrap();
    let path = resources
        .download(
            PLUGIN,
            &spec(vec!["http://127.0.0.1:1/unavailable".into()], &data),
            &Ctx::default(),
        )
        .unwrap();
    assert_eq!(fs::read(path).unwrap(), data);
    assert!(!part.exists());
}

#[test]
fn a_corrupt_complete_partial_file_is_closed_before_restarting() {
    let data = payload();
    let (url, requests, worker) = scripted(data.clone(), vec![Reply::Good]);
    let resources = store("corrupt-part");
    let (_, _, part) = resources.paths(PLUGIN, "model").unwrap();
    fs::create_dir_all(part.parent().unwrap()).unwrap();
    fs::write(&part, vec![0; data.len()]).unwrap();
    let path = resources
        .download(PLUGIN, &spec(vec![url], &data), &Ctx::default())
        .unwrap();
    worker.join().unwrap();
    assert_eq!(*requests.lock().unwrap(), vec![None]);
    assert_eq!(fs::read(&path).unwrap(), data);
    fs::remove_file(path).unwrap();
}

#[test]
fn retry_is_bounded_and_does_not_repeat_permanent_http_errors() {
    for (status, count) in [(503, SOURCE_ATTEMPTS), (404, 1)] {
        let data = payload();
        let (url, requests, worker) = scripted(data.clone(), vec![Reply::Status(status); count]);
        let resources = store(&format!("http-{status}"));
        let error = resources
            .download(PLUGIN, &spec(vec![url], &data), &Ctx::default())
            .unwrap_err();
        worker.join().unwrap();
        assert_eq!(error.code, "resource.http_status");
        assert_eq!(error.params["status"], status);
        assert_eq!(requests.lock().unwrap().len(), count);
    }
}

#[test]
fn cancellation_during_retry_keeps_an_unlocked_partial_file() {
    let data = payload();
    let (url, requests, worker) = scripted(data.clone(), vec![Reply::Short]);
    let resources = store("retry-cancel");
    let ctx = Ctx {
        cancel_on_retry: true,
        ..Ctx::default()
    };
    let error = resources
        .download(PLUGIN, &spec(vec![url], &data), &ctx)
        .unwrap_err();
    worker.join().unwrap();
    assert_eq!(error.code, "task.cancelled");
    assert_eq!(requests.lock().unwrap().len(), 1);
    let (_, _, part) = resources.paths(PLUGIN, "model").unwrap();
    let partial = fs::metadata(&part).unwrap().len();
    assert!(partial > 0 && partial <= (data.len() / 2) as u64);
    fs::remove_file(part).unwrap();
}
