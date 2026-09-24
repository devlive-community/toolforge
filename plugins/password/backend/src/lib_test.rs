use super::*;
use serde_json::json;

#[test]
fn dispatches_functions() {
    let tool = Password::default();
    let out = tool
        .call("generate", json!({"length": 16, "count": 3}))
        .unwrap();
    assert_eq!(out["passwords"].as_array().unwrap().len(), 3);
    let report = tool
        .call("analyze", json!({"password": "password"}))
        .unwrap();
    assert_eq!(report["score"], 0);
    assert!(tool.manifest().sensitive);
}
