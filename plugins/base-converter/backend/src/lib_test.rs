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
