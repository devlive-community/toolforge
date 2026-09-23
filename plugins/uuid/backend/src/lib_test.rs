use super::*;
use serde_json::json;

#[test]
fn dispatches_declared_functions() {
    let tool = UuidTool::default();
    let out = tool
        .call("generate", json!({"kind": "v4", "count": 3}))
        .unwrap();
    assert_eq!(out["ids"].as_array().unwrap().len(), 3);
    let info = tool
        .call("inspect", json!({"input": out["ids"][0]}))
        .unwrap();
    assert_eq!(info["version"], 4);
    for function in ["generate", "inspect"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
