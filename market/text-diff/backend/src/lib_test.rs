use super::*;
use serde_json::json;

#[test]
fn dispatches_diff() {
    let out = TextDiff::default()
        .call("diff", json!({"left": "a", "right": "b", "mode": "lines"}))
        .unwrap();
    assert_eq!(out["rows"][0]["tag"], "change");
    assert_eq!(out["identical"], false);
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&TextDiff::default()).unwrap();
    assert_eq!(checked, 1);
}
