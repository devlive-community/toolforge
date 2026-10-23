use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;

use super::*;

const MANIFEST: &str = include_str!("../fixture/manifest.json");

/// 编译测试夹具为 wasm32-wasip2 组件（每次测试进程只编译一次）
fn fixture() -> &'static [u8] {
    static WASM: OnceLock<Vec<u8>> = OnceLock::new();
    WASM.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target = root.join("../../target/wasm-fixture");
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--release",
                "--target",
                "wasm32-wasip2",
                "--manifest-path",
            ])
            .arg(root.join("fixture/Cargo.toml"))
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .expect("run cargo");
        assert!(
            status.success(),
            "building the fixture needs `rustup target add wasm32-wasip2`"
        );
        std::fs::read(target.join("wasm32-wasip2/release/tf_wasm_fixture.wasm"))
            .expect("fixture wasm")
    })
}

fn load() -> WasmPlugin {
    WasmPlugin::load(Manifest::from_static(MANIFEST), fixture()).unwrap()
}

#[derive(Default)]
struct Ctx {
    logs: Mutex<Vec<(String, Value)>>,
    progress: Mutex<Vec<(u64, u64)>>,
    stages: Mutex<Vec<String>>,
    cancel: AtomicBool,
}

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, code: &str, params: Value) {
        self.logs.lock().unwrap().push((code.to_owned(), params));
    }
    fn progress(&self, done: u64, total: u64) {
        self.progress.lock().unwrap().push((done, total));
    }
    fn stage(&self, code: &str) {
        self.stages.lock().unwrap().push(code.to_owned());
    }
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    fn open_file(&self, _path: &str) -> PluginResult<Box<dyn Read + Send>> {
        unreachable!("wasm plugins read files through the host import")
    }
    fn file_size(&self, _path: &str) -> PluginResult<u64> {
        unreachable!("wasm plugins read files through the host import")
    }
}

#[test]
fn calls_keep_instance_state_and_map_errors() {
    let plugin = load();
    assert_eq!(
        plugin.call("echo", json!({"a": 1})).unwrap(),
        json!({"args": {"a": 1}, "calls": 1})
    );
    assert_eq!(plugin.call("echo", json!(null)).unwrap()["calls"], 2);
    assert_eq!(
        plugin.call("sum", json!({"values": [1, 2, 39]})).unwrap(),
        json!(42)
    );
    let err = plugin.call("fail", json!({})).unwrap_err();
    assert_eq!(err.code, "fixture.failed");
    assert_eq!(err.params["reason"], "asked");
    assert_eq!(
        plugin.call("missing", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn passes_the_host_time_zone() {
    let tz = load().call("tz", json!({})).unwrap();
    assert_eq!(tz, json!(local_time_zone()));
    assert!(!local_time_zone().is_empty());
}

#[test]
fn panics_reset_the_instance() {
    let plugin = load();
    plugin.call("echo", json!({})).unwrap();
    let err = plugin.call("panic", json!({})).unwrap_err();
    assert_eq!(err.code, "plugin.crashed");
    // 新实例从头计数
    assert_eq!(plugin.call("echo", json!({})).unwrap()["calls"], 1);
}

#[test]
fn runaway_calls_time_out() {
    let plugin = load().with_call_timeout(Duration::from_millis(300));
    let started = std::time::Instant::now();
    assert_eq!(
        plugin.call("spin", json!({})).unwrap_err().code,
        "plugin.timeout"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(plugin.call("echo", json!({})).is_ok());
}

#[test]
fn tasks_forward_logs_progress_and_cancellation() {
    let plugin = load();
    let ctx = Ctx::default();
    let out = plugin.run_task("count", json!({"total": 3}), &ctx).unwrap();
    assert_eq!(out, json!({"counted": 3}));
    assert_eq!(*ctx.progress.lock().unwrap(), vec![(1, 3), (2, 3), (3, 3)]);
    assert_eq!(*ctx.stages.lock().unwrap(), vec!["fixture.counting"]);
    assert_eq!(
        ctx.logs.lock().unwrap()[1],
        ("fixture.tick".to_owned(), json!({"i": 2}))
    );

    let cancelled = Ctx::default();
    cancelled.cancel.store(true, Ordering::Relaxed);
    assert_eq!(
        plugin
            .run_task("count", json!({}), &cancelled)
            .unwrap_err()
            .code,
        "task.cancelled"
    );
}

#[test]
fn reads_files_only_with_permission() {
    let path = std::env::temp_dir().join(format!("tf-wasm-read-{}", std::process::id()));
    std::fs::write(&path, "你好 wasm").unwrap();
    let args = json!({ "path": path.to_string_lossy() });
    let plugin = load();
    let out = plugin
        .run_task("read", args.clone(), &Ctx::default())
        .unwrap();
    assert_eq!(out, json!({"size": 11, "text": "你好 wasm"}));

    let mut manifest = Manifest::from_static(MANIFEST);
    manifest.permissions.clear();
    let locked = WasmPlugin::load(manifest, fixture()).unwrap();
    let err = locked.run_task("read", args, &Ctx::default()).unwrap_err();
    assert_eq!(err.code, "plugin.permission_denied");
    std::fs::remove_file(path).unwrap();
}

/// 回显方法、请求头与请求体；/redirect 302 到 /echo
fn echo_server() -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let base = format!(
        "http://127.0.0.1:{}",
        server.server_addr().to_ip().unwrap().port()
    );
    let own = base.clone();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            if request.url() == "/redirect" {
                let to = tiny_http::Header::from_bytes("Location", format!("{own}/echo")).unwrap();
                let _ = request.respond(tiny_http::Response::empty(302).with_header(to));
                continue;
            }
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            let fixture = request
                .headers()
                .iter()
                .any(|h| h.field.as_str().as_str().eq_ignore_ascii_case("x-fixture"));
            let text = format!("{} {body} {fixture}", request.method());
            let _ = request.respond(tiny_http::Response::from_string(text));
        }
    });
    base
}

#[test]
fn sends_http_requests_through_the_host() {
    let base = echo_server();
    let plugin = load();
    let ctx = Ctx::default();
    let out = plugin
        .run_task("fetch", json!({ "url": format!("{base}/redirect") }), &ctx)
        .unwrap();
    assert_eq!(out["status"], 200);
    assert_eq!(out["reason"], "OK");
    // 302 按惯例改为不带正文的 GET，自定义请求头保留
    assert_eq!(out["body"], "GET  true");
    assert_eq!(out["redirects"], json!([format!("{base}/echo")]));
    let out = plugin
        .run_task("fetch", json!({ "url": format!("{base}/echo") }), &ctx)
        .unwrap();
    assert_eq!(out["body"], "POST ping true");
    // 取消的任务在读取响应前结束
    ctx.cancel.store(true, Ordering::SeqCst);
    let err = plugin
        .run_task("fetch", json!({ "url": format!("{base}/echo") }), &ctx)
        .unwrap_err();
    assert_eq!(err.code, "task.cancelled");
    let err = plugin
        .run_task("fetch", json!({ "url": "ftp://x" }), &Ctx::default())
        .unwrap_err();
    assert_eq!(err.code, "net.unsupported_scheme");
}

#[test]
fn streams_responses_through_the_host() {
    let base = echo_server();
    let plugin = load();
    let out = plugin
        .run_task(
            "stream",
            json!({ "url": format!("{base}/echo") }),
            &Ctx::default(),
        )
        .unwrap();
    assert_eq!(out, json!({ "status": 200, "text": "GET  false" }));
    let mut manifest = Manifest::from_static(MANIFEST);
    manifest.permissions.clear();
    let locked = WasmPlugin::load(manifest, fixture()).unwrap();
    let err = locked
        .run_task(
            "stream",
            json!({ "url": format!("{base}/echo") }),
            &Ctx::default(),
        )
        .unwrap_err();
    assert_eq!(err.code, "plugin.permission_denied");
}

#[test]
fn listens_for_one_callback_and_releases_the_port() {
    let plugin = load();
    let port = plugin.call("listen", json!({})).unwrap().as_u64().unwrap();
    assert_eq!(plugin.call("poll", json!({})).unwrap(), Value::Null);
    let url = format!("http://127.0.0.1:{port}/cb?code=1&state=s");
    tf_net::ensure_crypto_provider();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap();
    assert_eq!(client.get(&url).send().unwrap().status(), 200);
    assert_eq!(plugin.call("poll", json!({})).unwrap(), "code=1&state=s");
    plugin.call("close", json!({})).unwrap();
    assert_eq!(
        plugin.call("poll", json!({})).unwrap_err().code,
        "fixture.no_callback"
    );
    // 关闭后端口释放
    std::net::TcpListener::bind(("127.0.0.1", port as u16)).unwrap();

    // 实例崩溃被丢弃时，监听也随之关闭
    let port = plugin.call("listen", json!({})).unwrap().as_u64().unwrap();
    assert!(plugin.call("panic", json!({})).is_err());
    std::net::TcpListener::bind(("127.0.0.1", port as u16)).unwrap();
}

#[test]
fn network_and_writes_need_permissions() {
    let path = std::env::temp_dir().join(format!("tf-wasm-write-{}", std::process::id()));
    let args = json!({ "path": path.to_string_lossy(), "text": "写入" });
    let plugin = load();
    plugin.call("write", args.clone()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "写入");
    std::fs::remove_file(&path).unwrap();

    let mut manifest = Manifest::from_static(MANIFEST);
    manifest.permissions.clear();
    let locked = WasmPlugin::load(manifest, fixture()).unwrap();
    let denied = |result: PluginResult<Value>| result.unwrap_err();
    let err = denied(locked.call("write", args));
    assert_eq!(
        (err.code.as_str(), &err.params["detail"]),
        ("plugin.permission_denied", &json!("fs:user-selected"))
    );
    let err = denied(locked.run_task(
        "fetch",
        json!({ "url": "http://127.0.0.1:9/" }),
        &Ctx::default(),
    ));
    assert_eq!(
        (err.code.as_str(), &err.params["detail"]),
        ("plugin.permission_denied", &json!("network"))
    );
    let err = denied(locked.call("listen", json!({})));
    assert_eq!(err.code, "plugin.permission_denied");
    assert!(!path.exists());
}

#[test]
fn detects_and_rejects_invalid_components() {
    let plugin = load();
    assert_eq!(
        plugin.detect("hello").map(|d| (d.score, d.label)),
        Some((77, "greeting".to_owned()))
    );
    assert!(plugin.detect("other").is_none());
    let err = WasmPlugin::load(Manifest::from_static(MANIFEST), b"not wasm")
        .err()
        .unwrap();
    assert_eq!(err.code, "plugin.load_failed");
}

/// 冒烟测试已编译的在线插件：
/// `TF_WASM_PLUGIN=market/<slug> TF_WASM_CALL='[task:]<函数> <JSON 参数>' cargo test -p tf-wasm calls_a_built_plugin -- --ignored --nocapture`
/// 需先 `cargo build --release --target wasm32-wasip2 -p tfm-<slug>`
#[test]
#[ignore]
fn calls_a_built_plugin() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(std::env::var("TF_WASM_PLUGIN").expect("set TF_WASM_PLUGIN"));
    let slug = dir.file_name().unwrap().to_string_lossy().replace('-', "_");
    let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    let wasm = std::fs::read(dir.join(format!(
        "../../target/wasm32-wasip2/release/tfm_{slug}.wasm"
    )))
    .unwrap();
    let plugin = WasmPlugin::load(Manifest::from_static(manifest.leak()), &wasm).unwrap();
    let call = std::env::var("TF_WASM_CALL").expect("set TF_WASM_CALL");
    let (function, args) = call.split_once(' ').unwrap_or((call.as_str(), "{}"));
    let args = serde_json::from_str(args).unwrap();
    let started = std::time::Instant::now();
    // 「task:<函数>」按任务运行
    let out = match function.strip_prefix("task:") {
        Some(task) => plugin.run_task(task, args, &Ctx::default()),
        None => plugin.call(function, args),
    };
    println!("{:?} in {:?}", out, started.elapsed());
    assert!(out.is_ok());
}

#[test]
fn file_scope_limits_reads_and_writes_to_granted_folders() {
    let root = std::env::temp_dir().join(format!("tf-wasm-scope-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (granted, outside) = (root.join("granted"), root.join("outside"));
    std::fs::create_dir_all(&granted).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(granted.join("in.txt"), "in").unwrap();
    std::fs::write(outside.join("out.txt"), "out").unwrap();
    let read = |p: &std::path::Path| json!({ "path": p.to_string_lossy() });

    let plugin = load();
    plugin.set_file_scope(FileScope::Roots {
        read: vec![granted.canonicalize().unwrap()],
        write: Vec::new(),
    });
    let ok = plugin
        .run_task("read", read(&granted.join("in.txt")), &Ctx::default())
        .unwrap();
    assert_eq!(ok["text"], "in");
    let err = plugin
        .run_task("read", read(&outside.join("out.txt")), &Ctx::default())
        .unwrap_err();
    assert_eq!(err.code, "plugin.path_denied");
    let escape = granted.join("../outside/out.txt");
    let err = plugin
        .run_task("read", read(&escape), &Ctx::default())
        .unwrap_err();
    assert_eq!(err.code, "plugin.path_denied", ".. cannot leave the folder");
    let write = json!({ "path": granted.join("new.txt").to_string_lossy(), "text": "x" });
    let err = plugin.call("write", write.clone()).unwrap_err();
    assert_eq!(
        err.code, "plugin.path_denied",
        "read-only folders cannot be written"
    );

    plugin.set_file_scope(FileScope::Roots {
        read: Vec::new(),
        write: vec![granted.canonicalize().unwrap()],
    });
    plugin.call("write", write).unwrap();
    assert_eq!(
        std::fs::read_to_string(granted.join("new.txt")).unwrap(),
        "x"
    );

    plugin.set_file_scope(FileScope::Any);
    plugin
        .run_task("read", read(&outside.join("out.txt")), &Ctx::default())
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
