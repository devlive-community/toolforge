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
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
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
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, _: Value) {
        self.logs.lock().unwrap().push(code.to_owned());
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
