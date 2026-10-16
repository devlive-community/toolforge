use super::*;
use serde_json::json;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

#[test]
fn inspect_returns_handle_and_stats() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = CspInspector::default();
    let out = plugin
        .call(
            "inspect",
            json!({
                "policies": "default-src 'self'",
                "documentUrl": "https://a.com/",
                "resources": [{ "type": "script", "url": "https://a.com/app.js" }],
            }),
        )
        .unwrap();
    assert!(out["handle"].as_u64().unwrap() >= 1);
    assert_eq!(out["resources"][0]["status"], "allowed");
    assert_eq!(out["stats"]["allowed"], 1);
}

#[test]
fn export_requires_an_inspection() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let plugin = CspInspector::default();
    let error = plugin
        .call("export", json!({ "format": "markdown" }))
        .unwrap_err();
    // 先做一次 inspect 再导出（静态缓存在进程内共享，这里直接验证流程）
    let _ = plugin.call("export", json!({ "format": "json" }));
    assert!(matches!(error.code.as_str(), "csp.no_result"));
    plugin
        .call(
            "inspect",
            json!({ "policies": "default-src 'none'", "resources": [] }),
        )
        .unwrap();
    let markdown = plugin
        .call("export", json!({ "format": "markdown" }))
        .unwrap();
    assert_eq!(markdown["ext"], "md");
    assert!(
        markdown["content"]
            .as_str()
            .unwrap()
            .contains("# CSP Inspection")
    );
    let json_report = plugin.call("export", json!({ "format": "json" })).unwrap();
    assert_eq!(json_report["ext"], "json");
}

#[test]
fn rejects_empty_and_oversized_input() {
    let plugin = CspInspector::default();
    let error = plugin
        .call("inspect", json!({ "policies": "  \n  " }))
        .unwrap_err();
    assert_eq!(error.code, "csp.no_policies");
    let big = "a".repeat(policy::MAX_TEXT + 1);
    let error = plugin
        .call("inspect", json!({ "policies": big }))
        .unwrap_err();
    assert_eq!(error.code, "csp.too_large");
    let error = plugin.call("nope", json!({})).unwrap_err();
    assert_eq!(error.code, "plugin.function_not_found");
}

#[test]
fn rejects_invalid_document_url() {
    let plugin = CspInspector::default();
    let error = plugin
        .call(
            "inspect",
            json!({ "policies": "default-src 'self'", "documentUrl": "http://[bad" }),
        )
        .unwrap_err();
    assert_eq!(error.code, "csp.invalid_url");
}
