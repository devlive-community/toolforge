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
