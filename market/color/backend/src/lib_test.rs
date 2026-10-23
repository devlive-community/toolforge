use super::*;
use serde_json::json;

#[test]
fn dispatches_convert() {
    let out = ColorTool::default()
        .call("convert", json!({"input": "white"}))
        .unwrap();
    assert_eq!(out["hex"], "#ffffff");
    assert_eq!(out["dark"], false);
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&ColorTool::default()).unwrap();
    assert_eq!(checked, 1);
}
