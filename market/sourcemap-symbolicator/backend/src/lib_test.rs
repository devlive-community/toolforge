use super::*;
use serde_json::json;
use std::io::Cursor;
use std::sync::Mutex;

/// 静态缓存在同进程内共享，涉及句柄的用例串行执行
static LOCK: Mutex<()> = Mutex::new(());

const SIMPLE: &str = r#"{
  "version": 3,
  "file": "app.min.js",
  "sources": ["src/a.ts"],
  "sourcesContent": ["line0\nline1\nline2\nexport const x = 1\nline4\nline5\nline6"],
  "names": [],
  "mappings": "AAAA"
}"#;

struct FileCtx {
    content: Vec<u8>,
}

impl TaskContext for FileCtx {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Ok(Box::new(Cursor::new(self.content.clone())))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Ok(self.content.len() as u64)
    }
}

#[test]
fn parse_returns_overview_and_expires_old_handles() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = SourceMapSymbolicator::default();
    let first = plugin.call("parse", json!({ "text": SIMPLE })).unwrap();
    assert_eq!(first["kind"], "regular");
    assert_eq!(first["sources"].as_array().unwrap().len(), 1);
    assert_eq!(first["sources"][0]["has_content"], true);
    let first_handle = first["handle"].as_u64().unwrap();

    let second = plugin.call("parse", json!({ "text": SIMPLE })).unwrap();
    assert!(second["handle"].as_u64().unwrap() > first_handle);

    let error = plugin
        .call(
            "symbolicate",
            json!({ "handle": first_handle, "stack": "at f (app.min.js:1:1)" }),
        )
        .unwrap_err();
    assert_eq!(error.code, "sourcemap.expired");
}

#[test]
fn symbolicate_then_export() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = SourceMapSymbolicator::default();
    let overview = plugin.call("parse", json!({ "text": SIMPLE })).unwrap();
    let handle = overview["handle"].as_u64().unwrap();

    let result = plugin
        .call(
            "symbolicate",
            json!({
                "handle": handle,
                "stack": "at doWork (http://x/app.min.js:1:1)",
                "asset": "app.min.js",
            }),
        )
        .unwrap();
    assert_eq!(result["frames"][0]["status"], "resolved");
    assert_eq!(result["frames"][0]["resolved"]["source"], "src/a.ts");

    let markdown = plugin
        .call("export", json!({ "handle": handle, "format": "markdown" }))
        .unwrap();
    assert_eq!(markdown["ext"], "md");
    assert!(
        markdown["content"]
            .as_str()
            .unwrap()
            .contains("# Source Map Symbolication")
    );

    let json_report = plugin
        .call("export", json!({ "handle": handle, "format": "json" }))
        .unwrap();
    assert_eq!(json_report["ext"], "json");
    assert!(
        json_report["content"]
            .as_str()
            .unwrap()
            .contains("\"resolved\"")
    );
}

#[test]
fn export_requires_a_result() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = SourceMapSymbolicator::default();
    let error = plugin
        .call("export", json!({ "handle": 999, "format": "markdown" }))
        .unwrap_err();
    assert_eq!(error.code, "sourcemap.no_result");
}

#[test]
fn rejects_oversized_and_unknown_input() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = SourceMapSymbolicator::default();
    let big = "x".repeat(MAX_STACK + 1);
    let error = plugin
        .call("symbolicate", json!({ "handle": 1, "stack": big }))
        .unwrap_err();
    assert_eq!(error.code, "sourcemap.stack_too_large");
    let error = plugin.call("nope", json!({})).unwrap_err();
    assert_eq!(error.code, "plugin.function_not_found");
}

#[test]
fn load_task_reads_and_parses_files() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = SourceMapSymbolicator::default();
    let ctx = FileCtx {
        content: SIMPLE.as_bytes().to_vec(),
    };
    let overview = plugin
        .run_task("load", json!({ "path": "/virtual/app.min.js.map" }), &ctx)
        .unwrap();
    assert_eq!(overview["kind"], "regular");
    assert_eq!(overview["file"], "app.min.js");
    assert_eq!(overview["segments"], 1);
}

#[test]
fn load_task_reports_missing_files() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    struct Missing;
    impl TaskContext for Missing {
        fn log(&self, _: LogLevel, _: &str, _: Value) {}
        fn progress(&self, _: u64, _: u64) {}
        fn stage(&self, _: &str) {}
        fn is_cancelled(&self) -> bool {
            false
        }
        fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
            Err(PluginError::new("fs.not_found"))
        }
        fn file_size(&self, _: &str) -> PluginResult<u64> {
            Err(PluginError::new("fs.not_found"))
        }
    }
    let plugin = SourceMapSymbolicator::default();
    let error = plugin
        .run_task("load", json!({ "path": "/missing.map" }), &Missing)
        .unwrap_err();
    assert_eq!(error.code, "fs.not_found");
}
