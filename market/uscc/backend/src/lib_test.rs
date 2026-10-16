use super::*;
use serde_json::json;

#[test]
fn dispatches_and_detects() {
    let tool = Uscc::default();
    let out = tool
        .call("validate", json!({"input": "91350100M000100Y43"}))
        .unwrap();
    assert_eq!(out["summary"]["valid"], 1);
    assert_eq!(out["rows"][0]["status"], "valid");
    assert!(tool.detect("91350100M000100Y43").is_some());
    assert!(tool.detect("91350100M000100Y44").is_none());
    for function in ["validate", "export"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
