use super::*;
use serde_json::json;

#[test]
fn dispatches_all_functions() {
    let tool = TextTools::default();
    let cases = tool.call("cases", json!({"input": "a b"})).unwrap();
    assert!(cases.as_array().unwrap().len() > 10);
    let lines = tool
        .call("lines", json!({"input": "b\na", "sort": "asc"}))
        .unwrap();
    assert_eq!(lines["output"], "a\nb");
    let stats = tool.call("stats", json!({"input": "a b"})).unwrap();
    assert_eq!(stats["words"], 2);
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&TextTools::default()).unwrap();
    assert_eq!(checked, 3);
}
