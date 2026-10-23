use super::*;
use serde_json::json;

#[test]
fn dispatches_test_and_replace() {
    let tool = Regex::default();
    let out = tool
        .call(
            "test",
            json!({"pattern": "o", "flags": {"caseInsensitive": true}, "input": "fOo"}),
        )
        .unwrap();
    assert_eq!(out["count"], 2);
    let replaced = tool
        .call(
            "replace",
            json!({"pattern": "o", "input": "foo", "replacement": "0"}),
        )
        .unwrap();
    assert_eq!(replaced["output"], "f00");
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked = tf_plugin_api::interface::check_declared_interfaces(&Regex::default()).unwrap();
    assert_eq!(checked, 3);
}
