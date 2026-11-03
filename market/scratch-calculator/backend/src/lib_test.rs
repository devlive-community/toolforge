use super::*;
use serde_json::json;

#[test]
fn dispatches_evaluate() {
    let out = ScratchCalculator::default()
        .call("evaluate", json!({"input": "1 + 2"}))
        .unwrap();
    assert_eq!(out["lines"][0]["value"], 3.0);
    assert_eq!(out["last"], 3.0);
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&ScratchCalculator::default()).unwrap();
    assert_eq!(checked, 1);
}
