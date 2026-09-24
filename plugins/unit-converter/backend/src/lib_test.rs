use super::*;
use serde_json::json;

#[test]
fn dispatches_functions() {
    let tool = UnitConverter::default();
    let catalog = tool.call("catalog", json!({})).unwrap();
    assert_eq!(catalog[0]["id"], "length");
    let out = tool
        .call(
            "convert",
            json!({"category": "length", "from": "km", "value": "1.5"}),
        )
        .unwrap();
    assert_eq!(out["input"], "1.5");
}
