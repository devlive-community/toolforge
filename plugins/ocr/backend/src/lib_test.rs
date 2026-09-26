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
