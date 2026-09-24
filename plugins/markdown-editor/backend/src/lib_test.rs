use super::*;
use serde_json::json;

#[test]
fn dispatches_functions() {
    let tool = MarkdownEditor::default();
    let out = tool.call("render", json!({ "source": "# Hi" })).unwrap();
    assert_eq!(out["title"], "Hi");
    assert_eq!(out["stats"]["readingMinutes"], 1);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}
