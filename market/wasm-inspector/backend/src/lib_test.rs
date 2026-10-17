use std::io::Read;
use std::sync::Mutex;

use super::*;
use crate::fixtures::sample;
use serde_json::json;

/// 用内存中的字节模拟宿主文件读取
struct Ctx {
    bytes: Vec<u8>,
    progress: Mutex<Vec<(u64, u64)>>,
    cancel: bool,
}

impl Ctx {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            progress: Mutex::new(Vec::new()),
            cancel: false,
        }
    }
}

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, _code: &str, _params: Value) {}
    fn progress(&self, done: u64, total: u64) {
        self.progress.lock().unwrap().push((done, total));
    }
    fn stage(&self, _code: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancel
    }
    fn open_file(&self, _path: &str) -> PluginResult<Box<dyn Read + Send>> {
        Ok(Box::new(std::io::Cursor::new(self.bytes.clone())))
    }
    fn file_size(&self, _path: &str) -> PluginResult<u64> {
        Ok(self.bytes.len() as u64)
    }
}

/// 缓存是进程级的，涉及缓存的断言放在同一个测试里串行执行
#[test]
fn loads_lists_prints_and_exports() {
    let tool = WasmInspector::default();
    let ctx = Ctx::new(sample());
    let size = ctx.bytes.len() as u64;
    let out = tool
        .run_task("load", json!({ "path": "/tmp/app.wasm" }), &ctx)
        .unwrap();
    let token = out["token"].as_u64().unwrap();
    assert_eq!(out["name"], "app.wasm");
    assert_eq!(out["version"], 1);
    assert_eq!(out["validation"]["status"], "valid");
    assert_eq!(out["sizes"]["total"], size);
    assert_eq!(out["imports"], 2);
    assert_eq!(out["exports"], 3);
    assert_eq!(out["largest"][0]["name"], "add");
    assert_eq!(ctx.progress.lock().unwrap().last(), Some(&(size, size)));

    let page = tool
        .call(
            "list",
            json!({ "token": token, "kind": "functions", "sort": "size", "limit": 1 }),
        )
        .unwrap();
    assert_eq!(page["total"], 2);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["name"], "add");
    let found = tool
        .call(
            "list",
            json!({ "token": token, "kind": "exports", "query": "COUNT" }),
        )
        .unwrap();
    assert_eq!(found["total"], 1);
    assert_eq!(found["items"][0]["kind"], "global");
    let imports = tool
        .call(
            "list",
            json!({ "token": token, "kind": "imports", "offset": 1 }),
        )
        .unwrap();
    assert_eq!(imports["total"], 2);
    assert_eq!(imports["items"][0]["name"], "mem");
    assert_eq!(
        tool.call("list", json!({ "token": token, "kind": "nope" }))
            .unwrap_err()
            .code,
        "wasm.invalid_kind"
    );

    // WAT 生成之前不能查看函数或导出 WAT
    assert_eq!(
        tool.call("function_wat", json!({ "token": token, "index": 1 }))
            .unwrap_err()
            .code,
        "wasm.wat_not_ready"
    );
    let wat = tool
        .run_task("wat", json!({ "token": token }), &ctx)
        .unwrap();
    assert_eq!(wat["truncated"], false);
    assert_eq!(wat["functions"], 2);
    assert!(wat["outline"].as_str().unwrap().contains("(module"));
    let f = tool
        .call("function_wat", json!({ "token": token, "index": 1 }))
        .unwrap();
    assert!(f["text"].as_str().unwrap().contains("i32.add"));
    // 导入函数没有函数体
    assert_eq!(
        tool.call("function_wat", json!({ "token": token, "index": 0 }))
            .unwrap_err()
            .code,
        "wasm.function_not_found"
    );

    let text = tool
        .call("export", json!({ "token": token, "format": "wat" }))
        .unwrap();
    assert_eq!(text["ext"], "wat");
    let report = tool
        .call("export", json!({ "token": token, "format": "json" }))
        .unwrap();
    let report: Value = serde_json::from_str(report["content"].as_str().unwrap()).unwrap();
    assert_eq!(report["file"], "app.wasm");
    assert_eq!(report["functions"].as_array().unwrap().len(), 2);
    assert_eq!(report["summary"]["validation"]["status"], "valid");

    // 打开新文件后旧 token 失效
    tool.run_task("load", json!({ "path": "b.wasm" }), &ctx)
        .unwrap();
    assert_eq!(
        tool.call("list", json!({ "token": token, "kind": "imports" }))
            .unwrap_err()
            .code,
        "wasm.expired"
    );
}

#[test]
fn reports_unusable_files() {
    let tool = WasmInspector::default();
    let err = tool
        .run_task(
            "load",
            json!({ "path": "x.wasm" }),
            &Ctx::new(b"MZ\x90\0".to_vec()),
        )
        .unwrap_err();
    assert_eq!(err.code, "wasm.bad_magic");
    assert_eq!(err.params["offset"], 0);

    let component = wasm_encoder::Component::new().finish();
    let err = tool
        .run_task("load", json!({ "path": "c.wasm" }), &Ctx::new(component))
        .unwrap_err();
    assert_eq!(err.code, "wasm.component");

    let mut ctx = Ctx::new(sample());
    ctx.cancel = true;
    let err = tool
        .run_task("load", json!({ "path": "a.wasm" }), &ctx)
        .unwrap_err();
    assert_eq!(err.code, "task.cancelled");
}

#[test]
fn manifest_declares_every_function() {
    let tool = WasmInspector::default();
    for name in ["load", "list", "wat", "function_wat", "export"] {
        assert!(tool.manifest().functions.contains_key(name), "{name}");
    }
    assert!(tool.manifest().functions["load"].task);
    assert!(tool.manifest().functions["wat"].task);
}
