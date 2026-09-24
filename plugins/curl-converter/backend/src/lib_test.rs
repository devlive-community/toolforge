use super::*;
use serde_json::json;

#[test]
fn converts_and_reports() {
    let tool = CurlConverter::default();
    let out = tool
        .call(
            "convert",
            json!({ "command": "curl https://a.b --bogus", "language": "python" }),
        )
        .unwrap();
    assert!(out["code"].as_str().unwrap().contains("requests.get(url)"));
    assert_eq!(out["request"]["method"], "GET");
    assert_eq!(out["request"]["body"]["kind"], "none");
    assert_eq!(out["warnings"][0]["code"], "curl.unknown_option");

    let err = |command: &str, language: &str| {
        tool.call(
            "convert",
            json!({ "command": command, "language": language }),
        )
        .unwrap_err()
        .code
    };
    assert_eq!(err("  ", "python"), "curl.empty");
    assert_eq!(err("curl x", "cobol"), "curl.unknown_language");
    assert_eq!(err("curl 'x", "go"), "curl.unterminated_quote");
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}
