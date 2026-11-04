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
    assert_eq!(checked, 4);
}

#[test]
fn export_styled_inlines_everything() {
    let out = MarkdownEditor::default()
        .call(
            "export_styled",
            json!({"source": "# 标题\n\n正文 **加粗** 与 `代码`\n\n- 列表项"}),
        )
        .unwrap();
    let html = out["html"].as_str().unwrap();
    assert!(
        html.contains("style="),
        "styles are injected inline: {html}"
    );
    assert!(
        html.contains("<strong style=\""),
        "strong gets inline style"
    );
    assert!(html.contains("<code style=\""), "code gets inline style");
    assert!(html.contains("<li style=\""), "li gets inline style");
    assert!(!html.contains("<style"), "no style blocks remain");
}

#[test]
fn export_styled_validates_color() {
    let err = MarkdownEditor::default()
        .call(
            "export_styled",
            json!({"source": "# x", "accent": "javascript:alert(1)"}),
        )
        .unwrap_err();
    assert_eq!(err.code, "styled.bad_color");
}
