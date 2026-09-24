use super::*;
use serde_json::json;

#[test]
fn dispatches_generate() {
    let out = QrCodeTool::default()
        .call("generate", json!({"text": "hi", "ecc": "H"}))
        .unwrap();
    assert_eq!(out["version"], 1);
    assert_eq!(out["modules"], 21);
}
