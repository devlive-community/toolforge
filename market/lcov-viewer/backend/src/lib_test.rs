use std::io::Read;
use std::sync::Mutex;

use super::*;
use serde_json::json;

/// 用内存中的文本模拟宿主文件读取
struct Ctx {
    text: &'static str,
    progress: Mutex<Vec<(u64, u64)>>,
    cancel: bool,
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
        Ok(Box::new(std::io::Cursor::new(self.text.as_bytes())))
    }
    fn file_size(&self, _path: &str) -> PluginResult<u64> {
        Ok(self.text.len() as u64)
    }
}

/// 缓存是进程级的，涉及缓存的断言放在同一个测试里串行执行
#[test]
fn loads_caches_and_serves_details() {
    let tool = LcovViewer::default();
    let ctx = Ctx {
        text: "SF:/r/a.rs\nDA:1,1\nDA:2,0\nend_of_record\n",
        progress: Mutex::new(Vec::new()),
        cancel: false,
    };
    let loaded = tool
        .run_task("load", json!({ "path": "lcov.info" }), &ctx)
        .unwrap();
    let token = loaded["token"].as_u64().unwrap();
    assert_eq!(loaded["totals"]["lines"]["found"], 2);
    assert_eq!(ctx.progress.lock().unwrap().last(), Some(&(39, 39)));

    let detail = tool
        .call(
            "detail",
            json!({ "token": token, "file": "/r/a.rs", "source": "x\ny\n" }),
        )
        .unwrap();
    assert_eq!(detail["lines"][1]["hits"], 0);
    assert_eq!(detail["metrics"]["lines"]["found"], 2);
    let csv = tool
        .call("export", json!({ "token": token, "format": "csv" }))
        .unwrap();
    assert!(csv.as_str().unwrap().contains("/r/a.rs,1,2,50.00%"));
    assert_eq!(
        tool.call("detail", json!({ "token": token, "file": "/nope" }))
            .unwrap_err()
            .code,
        "lcov.file_not_found"
    );

    // 新的解析替换缓存，旧 token 失效
    let parsed = tool
        .call("parse", json!({ "text": "SF:b\nDA:1,1\nend_of_record\n" }))
        .unwrap();
    assert_ne!(parsed["token"].as_u64().unwrap(), token);
    assert_eq!(
        tool.call("export", json!({ "token": token, "format": "csv" }))
            .unwrap_err()
            .code,
        "lcov.expired"
    );
    assert_eq!(
        tool.call("parse", json!({ "text": "TN:x\n" }))
            .unwrap_err()
            .code,
        "lcov.empty"
    );

    let resolved = tool
        .call("resolve", json!({ "path": "src/a.rs", "root": "/r" }))
        .unwrap();
    assert_eq!(resolved["path"], "/r/src/a.rs");
    for function in ["load", "parse", "detail", "resolve", "export"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
