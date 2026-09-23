use super::*;
use serde_json::json;

fn format_with(value: serde_json::Value) -> PluginResult<Output> {
    format(serde_json::from_value(value).unwrap())
}

#[test]
fn formats_with_uppercase_keywords_by_default() {
    let out = format_with(json!({"input": "select id, name from users where id = 1"})).unwrap();
    assert_eq!(
        out.output,
        "SELECT\n  id,\n  name\nFROM\n  users\nWHERE\n  id = 1"
    );
    assert_eq!(out.stats.statements, 1);
}

#[test]
fn keyword_case_and_indent_options() {
    let lower =
        format_with(json!({"input": "SELECT a FROM t", "keywordCase": "lower", "indent": "4"}))
            .unwrap();
    assert_eq!(lower.output, "select\n    a\nfrom\n    t");
    let preserve = format_with(
        json!({"input": "Select a From t", "keywordCase": "preserve", "indent": "tab"}),
    )
    .unwrap();
    assert_eq!(preserve.output, "Select\n\ta\nFrom\n\tt");
}

#[test]
fn compact_keeps_short_argument_lists_inline() {
    let out = format_with(json!({"input": "select a, b, c from t", "compact": true})).unwrap();
    assert!(out.output.starts_with("SELECT a, b, c"));
}

#[test]
fn separates_multiple_queries() {
    let out =
        format_with(json!({"input": "select 1; select 2;", "linesBetweenQueries": 2})).unwrap();
    assert_eq!(out.stats.statements, 2);
    assert_eq!(out.output, "SELECT\n  1;\n\nSELECT\n  2;");
}

#[test]
fn rejects_empty_input() {
    assert_eq!(
        format_with(json!({"input": "  "})).unwrap_err().code,
        "sql.empty"
    );
}
