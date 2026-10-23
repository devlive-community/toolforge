use super::*;
use serde_json::json;

#[test]
fn dispatches_functions() {
    let tool = Password::default();
    let out = tool
        .call("generate", json!({"length": 16, "count": 3}))
        .unwrap();
    assert_eq!(out["passwords"].as_array().unwrap().len(), 3);
    let phrase = tool
        .call("passphrase", json!({"words": 4, "wordlist": "zh"}))
        .unwrap();
    assert_eq!(phrase["hints"].as_array().unwrap().len(), 1);
    let report = tool
        .call("analyze", json!({"password": "password"}))
        .unwrap();
    assert_eq!(report["score"], 0);
    assert!(tool.manifest().sensitive);
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&Password::default()).unwrap();
    assert_eq!(checked, 3);
}
