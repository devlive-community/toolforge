use serde_json::json;

use super::*;

#[test]
fn detects_hidden_characters_on_the_clipboard() {
    let plugin = UnicodeInspector::default();
    assert_eq!(plugin.detect("admin\u{202e}txt.exe").unwrap().label, "bidi");
    let hidden = plugin.detect("pass\u{200b}word").unwrap();
    assert_eq!(
        (hidden.label.as_str(), hidden.params["count"].as_u64()),
        ("hidden", Some(1))
    );
    assert!(
        plugin.detect("👨‍👩‍👧 family ❤️").is_none(),
        "emoji joiners are normal"
    );
    assert!(plugin.detect("plain text").is_none());
}

#[test]
fn exposes_functions() {
    let plugin = UnicodeInspector::default();
    let analysis = plugin
        .call("analyze", json!({ "text": "a\u{200b}b" }))
        .unwrap();
    assert_eq!(analysis["counts"]["invisible"], 1);
    let cleaned = plugin
        .call("clean", json!({ "text": "a\u{200b}b", "form": "nfc" }))
        .unwrap();
    assert_eq!(cleaned["text"], "ab");
    let found = plugin.call("lookup", json!({ "query": "U+00E9" })).unwrap();
    assert_eq!(found[0]["name"], "LATIN SMALL LETTER E WITH ACUTE");
    let big = "x".repeat(MAX_TEXT + 1);
    assert_eq!(
        plugin
            .call("analyze", json!({ "text": big }))
            .unwrap_err()
            .code,
        "unicode.too_large"
    );
}
