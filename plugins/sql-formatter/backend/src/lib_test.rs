use super::*;
use serde_json::json;

#[test]
fn dispatches_format_and_minify() {
    let tool = SqlFormatter::default();
    let formatted = tool.call("format", json!({"input": "select 1"})).unwrap();
    assert_eq!(formatted["output"], "SELECT\n  1");
    let minified = tool
        .call("minify", json!({"input": "select\n  1"}))
        .unwrap();
    assert_eq!(minified["output"], "select 1");
}
