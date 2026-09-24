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
