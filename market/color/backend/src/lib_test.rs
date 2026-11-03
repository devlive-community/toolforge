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
    assert_eq!(checked, 2);
}

#[test]
fn dispatches_palette() {
    let out = ColorTool::default()
        .call("palette", json!({"query": "朱砂"}))
        .unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["groups"][0]["colors"][0]["hex"], "#ff461f");
}
