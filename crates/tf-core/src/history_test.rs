use serde_json::json;

use super::*;

#[test]
fn picks_named_input_then_longest_string() {
    assert_eq!(
        primary_text(&json!({"mode": "json", "input": "{\"a\":1}"})).as_deref(),
        Some("{\"a\":1}")
    );
    assert_eq!(
        primary_text(&json!({"pattern": "", "flags": {}, "input": "abc"})).as_deref(),
        Some("abc")
    );
    assert_eq!(
        primary_text(&json!({"kind": "v4", "name": "toolforge.dev"})).as_deref(),
        Some("toolforge.dev")
    );
    assert_eq!(primary_text(&json!({"count": 3, "input": "   "})), None);
    assert_eq!(primary_text(&json!([1, 2])), None);
}

#[test]
fn truncates_long_input_on_char_boundary() {
    let long = "中".repeat(MAX_INPUT);
    let text = primary_text(&json!({ "input": long })).unwrap();
    assert!(text.len() <= MAX_INPUT);
    assert!(text.chars().all(|c| c == '中'));
}

#[test]
fn previews_output() {
    assert_eq!(
        output_preview(&json!({"output": "  hi  ", "elapsedMs": 1})).as_deref(),
        Some("hi")
    );
    assert_eq!(output_preview(&json!("plain")).as_deref(), Some("plain"));
    assert_eq!(output_preview(&json!({"count": 1})), None);
    let preview = output_preview(&json!({"output": "x".repeat(600)})).unwrap();
    assert_eq!(preview.chars().count(), OUTPUT_PREVIEW + 1);
}
