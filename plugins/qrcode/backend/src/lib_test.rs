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

#[test]
fn offers_to_read_clipboard_images() {
    let tool = QrCodeTool::default();
    assert_eq!(tool.detect_image(400, 300).unwrap().label, "image");
    assert!(tool.detect_image(10, 300).is_none());
}
