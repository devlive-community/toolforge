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
        "repair",
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
fn repair_handles_issue_353_without_relaxing_validation() {
    let source = "{'name':'ToolForge',, enabled: True, missing: None, tags: ['a', 'b',],}";
    assert!(call("validate", json!({ "input": source })).is_err());
    assert!(call("format", json!({ "input": source, "mode": "json5" })).is_err());
    let result = call("repair", json!({ "input": source })).unwrap();
    let output: Value = serde_json::from_str(result["output"].as_str().unwrap()).unwrap();
    assert_eq!(
        output,
        json!({ "name": "ToolForge", "enabled": true, "missing": null, "tags": ["a", "b"] })
    );
    assert_eq!(result["changed"], true);
    for code in [
        "single_quotes",
        "extra_commas",
        "python_literals",
        "unquoted_keys",
    ] {
        assert!(
            result["repairs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == format!("json.repair.{code}")
                    && item["count"].as_u64().unwrap() > 0)
        );
    }
}

#[test]
fn repair_result_supports_indentation_sorting_stats_and_tree() {
    let plugin = JsonFormatter::default();
    let result = plugin
        .call(
            "repair",
            json!({ "input": "{z:None,a:'None, True'}", "indent": "4", "sortKeys": true }),
        )
        .unwrap();
    assert_eq!(
        result["output"],
        "{\n    \"a\": \"None, True\",\n    \"z\": null\n}"
    );
    assert_eq!(result["stats"]["nulls"], 1);
    assert_eq!(result["stats"]["strings"], 1);
    let page = plugin
        .call(
            "tree_children",
            json!({ "docId": result["docId"], "path": [] }),
        )
        .unwrap();
    assert_eq!(page["nodes"][0]["key"], "a");
    assert_eq!(page["nodes"][1]["key"], "z");
}

#[test]
fn repair_keeps_valid_numbers_exact_and_does_not_report_formatting_as_a_repair() {
    let result = call("repair", json!({ "input": "{\"n\":12345678901234567890123456789,\"decimal\":1.10,\"exponent\":1e9999}" })).unwrap();
    let output = result["output"].as_str().unwrap();
    assert!(output.contains("12345678901234567890123456789"));
    assert!(output.contains("1.10"));
    assert!(output.contains("1e+9999"));
    assert_eq!(result["changed"], false);
    assert_eq!(result["repairs"], json!([]));
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

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&JsonFormatter::default()).unwrap();
    assert_eq!(checked, 9);
}
