use serde_json::json;

use super::*;

fn messages() -> Messages {
    Messages::new("en-US").with_plugin(json!({ "api": { "col": { "algo": "Algorithm" } } }))
}

#[test]
fn text_hint_prints_the_value_as_is() {
    let result = json!({ "output": "{\n  \"a\": 1\n}\n", "elapsedMs": 1.2 });
    let hint = json!({ "text": "/output" });
    assert_eq!(
        render(&result, Some(&hint), &messages()),
        "{\n  \"a\": 1\n}"
    );
}

#[test]
fn table_hint_uses_translated_columns() {
    let result = json!({ "results": [
        { "algorithm": "md5", "digest": "abc" },
        { "algorithm": "sha256", "digest": "0123456789" }
    ]});
    let hint = json!({ "table": "/results", "columns": [
        { "key": "algorithm", "label": "i18n:api.col.algo" },
        { "key": "digest", "label": "Digest" }
    ]});
    assert_eq!(
        render(&result, Some(&hint), &messages()),
        "Algorithm  Digest\nmd5        abc\nsha256     0123456789"
    );
}

#[test]
fn missing_hint_target_falls_back_to_generic_rendering() {
    let result = json!({ "value": 1 });
    let hint = json!({ "text": "/nothing" });
    assert_eq!(render(&result, Some(&hint), &messages()), "value: 1");
}

#[test]
fn generic_rendering_handles_nesting_and_rows() {
    let result = json!({
        "decimal": "255",
        "negative": false,
        "bits": null,
        "tags": ["a", "b"],
        "summary": { "ok": 2, "failed": 0 },
        "widths": [{ "bits": 8, "hex": "ff" }, { "bits": 16, "hex": "00ff" }]
    });
    assert_eq!(
        render(&result, None, &messages()),
        "decimal: 255\nnegative: false\nbits: -\ntags: a, b\nsummary:\n  ok: 2\n  failed: 0\n\
         widths:\n  bits  hex\n  8     ff\n  16    00ff"
    );
    assert_eq!(render(&json!("plain"), None, &messages()), "plain");
}

#[test]
fn width_counts_wide_characters_twice() {
    assert_eq!(width("abc"), 3);
    assert_eq!(width("算法"), 4);
    assert_eq!(
        table(
            &["算法".to_owned(), "x".to_owned()],
            &[vec!["md5".to_owned(), "1".to_owned()]]
        ),
        "算法  x\nmd5   1"
    );
}
