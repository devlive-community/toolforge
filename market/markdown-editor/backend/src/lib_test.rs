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

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&MarkdownEditor::default()).unwrap();
    assert_eq!(checked, 2);
}
