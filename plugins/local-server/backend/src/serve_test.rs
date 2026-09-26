use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde_json::Value;

use super::*;

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<(String, Value)>>,
    cancelled: AtomicBool,
}

impl TaskContext for Ctx {
    fn log(&self, _: LogLevel, code: &str, params: Value) {
        self.logs.lock().unwrap().push((code.to_owned(), params));
    }
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("fs.not_found"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.not_found"))
    }
    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        Err(PluginError::new("resource.missing").with("id", id))
    }
}

fn args(root: &str, port: u16) -> Args {
    serde_json::from_value(json!({ "root": root, "port": port })).unwrap()
}

fn get(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn rejects_missing_folders() {
    let ctx = Ctx::default();
    let err = run(args("/definitely/not/here", 18_080), &ctx).unwrap_err();
    assert_eq!(err.code, "server.not_directory");
    let file = std::env::temp_dir().join(format!("tfp-server-file-{}", std::process::id()));
    std::fs::write(&file, "x").unwrap();
    let err = run(args(&file.to_string_lossy(), 18_080), &ctx).unwrap_err();
    assert_eq!(err.code, "server.not_directory");
}

#[test]
fn serves_until_cancelled_and_logs_requests() {
    let dir = std::env::temp_dir().join(format!("tfp-server-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("hello.txt"), "hello world").unwrap();
    let port = addresses::free_port(20_000, false).unwrap();
    let ctx = Ctx::default();
    std::thread::scope(|scope| {
        let server = scope.spawn(|| run(args(&dir.to_string_lossy(), port), &ctx));
        // 等待端口开始监听
        let deadline = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(Instant::now() < deadline, "server did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        let ok = get(
            port,
            "GET /hello.txt HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert!(ok.ends_with("hello world"), "{ok}");
        let partial = get(
            port,
            "GET /hello.txt HTTP/1.1\r\nRange: bytes=6-\r\nConnection: close\r\n\r\n",
        );
        assert!(
            partial.starts_with("HTTP/1.1 206") && partial.ends_with("\r\n\r\nworld"),
            "{partial}"
        );
        let head = get(
            port,
            "HEAD /hello.txt HTTP/1.1\r\nConnection: close\r\n\r\n",
        );
        assert!(
            head.starts_with("HTTP/1.1 200") && head.ends_with("\r\n\r\n"),
            "{head}"
        );
        let missing = get(port, "GET /../secret HTTP/1.1\r\nConnection: close\r\n\r\n");
        assert!(missing.starts_with("HTTP/1.1 400"), "{missing}");
        ctx.cancelled.store(true, Ordering::Relaxed);
        assert_eq!(server.join().unwrap().unwrap_err().code, "task.cancelled");
    });
    let logs = ctx.logs.lock().unwrap();
    let codes: Vec<&str> = logs.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(codes.first(), Some(&"server.started"));
    assert_eq!(codes.last(), Some(&"server.stopped"));
    let requests: Vec<&Value> = logs
        .iter()
        .filter(|(c, _)| c == "server.request")
        .map(|(_, p)| p)
        .collect();
    assert_eq!(requests.len(), 4);
    let first = requests
        .iter()
        .find(|p| p["status"] == 200 && p["method"] == "GET")
        .unwrap();
    assert_eq!(
        (first["url"].as_str(), first["size"].as_u64()),
        (Some("/hello.txt"), Some(11))
    );
    assert_eq!(logs.last().unwrap().1["requests"], 4);
    // 停止后端口被释放
    assert!(addresses::bind(port, false).is_ok());
}

#[test]
fn shortens_long_urls() {
    assert_eq!(short("/a"), "/a");
    let long = format!("/{}", "é".repeat(400));
    let cut = short(&long);
    assert!(cut.ends_with('…') && cut.len() <= MAX_URL + 3);
}
