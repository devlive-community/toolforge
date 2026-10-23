use super::*;
use serde_json::json;

#[test]
fn pretty_supports_tab_and_four_spaces() {
    let value = json!({"a": 1});
    assert_eq!(pretty(&value, Indent::Tab).unwrap(), "{\n\t\"a\": 1\n}");
    assert_eq!(pretty(&value, Indent::Four).unwrap(), "{\n    \"a\": 1\n}");
}

#[test]
fn escape_and_unescape_roundtrip() {
    let raw = "line1\n\"quoted\"\t中文";
    let escaped = escape(raw).unwrap();
    assert_eq!(escaped, "line1\\n\\\"quoted\\\"\\t中文");
    assert_eq!(unescape(&escaped).unwrap(), raw);
    assert_eq!(unescape(&format!("\"{escaped}\"")).unwrap(), raw);
}

#[test]
fn unescape_reports_invalid_sequences() {
    assert_eq!(
        unescape("bad \\q").unwrap_err().code,
        "json.unescape_failed"
    );
}
