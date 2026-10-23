use super::*;
use serde_json::json;

#[test]
fn dispatches_convert() {
    let out = BaseConverter::default()
        .call("convert", json!({"input": "0xff"}))
        .unwrap();
    assert_eq!(out["decimal"], "255");
    assert_eq!(out["base"], 16);
}

#[test]
fn declared_interface_matches_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&BaseConverter::default()).unwrap();
    assert_eq!(checked, 1);
}
