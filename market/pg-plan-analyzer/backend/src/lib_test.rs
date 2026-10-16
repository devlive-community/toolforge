use super::*;
use serde_json::json;
use std::io::Cursor;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

const PLAN: &str = r#"[{ "Plan": { "Node Type": "Seq Scan", "Relation Name": "t", "Startup Cost": 0, "Total Cost": 10, "Plan Rows": 100, "Plan Width": 4, "Actual Total Time": 1.5, "Actual Rows": 100, "Actual Loops": 1 }, "Execution Time": 1.7 }]"#;

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
fn parse_returns_report_and_expires_old_handles() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = PgPlanAnalyzer::default();
    let first = plugin.call("parse", json!({ "text": PLAN })).unwrap();
    assert_eq!(first["statements"][0]["has_actual"], true);
    let first_handle = first["handle"].as_u64().unwrap();

    let second = plugin.call("parse", json!({ "text": PLAN })).unwrap();
    assert!(second["handle"].as_u64().unwrap() > first_handle);

    let error = plugin
        .call(
            "export",
            json!({ "handle": first_handle, "format": "markdown" }),
        )
        .unwrap_err();
    assert_eq!(error.code, "pg.no_result");
}

#[test]
fn export_markdown_and_json() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = PgPlanAnalyzer::default();
    let out = plugin.call("parse", json!({ "text": PLAN })).unwrap();
    let handle = out["handle"].as_u64().unwrap();
    let markdown = plugin
        .call("export", json!({ "handle": handle, "format": "markdown" }))
        .unwrap();
    assert_eq!(markdown["ext"], "md");
    assert!(
        markdown["content"]
            .as_str()
            .unwrap()
            .contains("# PostgreSQL Plan Analysis")
    );
    let json_report = plugin
        .call("export", json!({ "handle": handle, "format": "json" }))
        .unwrap();
    assert_eq!(json_report["ext"], "json");
}

#[test]
fn rejects_oversized_and_unknown_input() {
    let plugin = PgPlanAnalyzer::default();
    let big = "x".repeat(plan::MAX_TEXT + 1);
    let error = plugin.call("parse", json!({ "text": big })).unwrap_err();
    assert_eq!(error.code, "pg.too_large");
    let error = plugin.call("nope", json!({})).unwrap_err();
    assert_eq!(error.code, "plugin.function_not_found");
}

#[test]
fn load_task_reads_and_parses_files() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = PgPlanAnalyzer::default();
    let ctx = FileCtx {
        content: PLAN.as_bytes().to_vec(),
    };
    let report = plugin
        .run_task("load", json!({ "path": "/virtual/plan.json" }), &ctx)
        .unwrap();
    assert_eq!(report["statements"].as_array().unwrap().len(), 1);
    assert_eq!(report["hotspots"][0]["kind"], "time");
}
