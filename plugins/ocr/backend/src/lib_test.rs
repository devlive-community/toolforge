use super::*;
use serde_json::json;

#[test]
fn only_exposes_the_recognize_task() {
    let tool = Ocr::default();
    assert!(tool.manifest().functions["recognize"].task);
    assert_eq!(
        tool.call("recognize", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
    assert_eq!(tool.manifest().resources.len(), 2);
}

#[test]
fn offers_to_read_clipboard_images() {
    let tool = Ocr::default();
    assert_eq!(tool.detect_image(1440, 900).unwrap().score, 80);
    assert!(tool.detect_image(8, 8).is_none());
}
