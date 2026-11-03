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
    assert_eq!(checked, 2);
}

#[test]
fn dispatches_statistics() {
    let out = ScratchCalculator::default()
        .call("statistics", json!({"input": "2\n4\n9"}))
        .unwrap();
    assert_eq!(out["count"], 3);
    assert_eq!(out["sum"], 15.0);
    assert_eq!(out["median"], 4.0);
}

#[test]
fn evaluate_accepts_angle_mode() {
    let out = ScratchCalculator::default()
        .call("evaluate", json!({"input": "sin(30)", "angleMode": "deg"}))
        .unwrap();
    assert!((out["lines"][0]["value"].as_f64().unwrap() - 0.5).abs() < 1e-12);
}
