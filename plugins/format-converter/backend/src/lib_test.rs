use super::*;
use serde_json::json;

#[test]
fn dispatches_convert() {
    let out = FormatConverter::default()
        .call(
            "convert",
            json!({"input": "{\"a\": 1}", "from": "auto", "to": "yaml"}),
        )
        .unwrap();
    assert_eq!(out["output"], "a: 1\n");
    assert_eq!(out["from"], "json");
}
