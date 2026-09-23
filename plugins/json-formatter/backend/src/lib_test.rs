use super::*;
use serde_json::json;

fn call(function: &str, args: Value) -> PluginResult<Value> {
    JsonFormatter::default().call(function, args)
}

#[test]
fn manifest_declares_every_dispatched_function() {
    let plugin = JsonFormatter::default();
    for function in [
        "format",
        "minify",
        "validate",
        "escape",
        "unescape",
        "tree_children",
        "diff",
    ] {
        assert!(
            plugin.manifest().functions.contains_key(function),
            "{function}"
        );
    }
}

#[test]
fn format_returns_output_stats_and_doc() {
    let out = call("format", json!({"input": "{\"a\":[1,2]}", "indent": "2"})).unwrap();
    assert_eq!(out["output"], "{\n  \"a\": [\n    1,\n    2\n  ]\n}");
    assert_eq!(out["stats"]["numbers"], 2);
    assert!(out["docId"].as_u64().is_some());
}

#[test]
fn minify_removes_whitespace() {
    let out = call("minify", json!({"input": "{ \"a\" : 1 }"})).unwrap();
    assert_eq!(out["output"], "{\"a\":1}");
}

#[test]
fn format_can_sort_keys() {
    let out = call(
        "minify",
        json!({"input": "{\"b\":1,\"a\":{\"d\":1,\"c\":2}}", "sortKeys": true}),
    )
    .unwrap();
    assert_eq!(out["output"], "{\"a\":{\"c\":2,\"d\":1},\"b\":1}");
}

#[test]
fn preserves_key_order_and_number_precision() {
    let out = call(
        "minify",
        json!({"input": "{\"z\":1.10,\"a\":12345678901234567890123}"}),
    )
    .unwrap();
    assert_eq!(out["output"], "{\"z\":1.10,\"a\":12345678901234567890123}");
}

#[test]
fn syntax_error_reports_position() {
    let err = call("validate", json!({"input": "{\n  \"a\": 1,\n}"})).unwrap_err();
    assert_eq!(err.code, "json.syntax");
    assert_eq!(err.params["line"], 3);
}

#[test]
fn tree_children_reads_cached_document() {
    let plugin = JsonFormatter::default();
    let out = plugin
        .call("format", json!({"input": "{\"a\":{\"b\":true}}"}))
        .unwrap();
    let page = plugin
        .call(
            "tree_children",
            json!({"docId": out["docId"], "path": ["a"]}),
        )
        .unwrap();
    assert_eq!(page["kind"], "object");
    assert_eq!(page["nodes"][0]["key"], "b");
    assert_eq!(page["nodes"][0]["preview"], "true");
}

#[test]
fn rejects_unknown_function() {
    assert_eq!(
        call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}
