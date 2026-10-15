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

#[test]
fn public_parser_matches_converter_without_running_or_reading_anything() {
    let command = "curl https://example.com -d @/definitely/not/read -H 'X-Literal: $(printenv)'";
    let (request, warnings) = parse_command(command).unwrap();
    assert!(matches!(request.body, Body::File { ref path } if path == "/definitely/not/read"));
    assert_eq!(request.header("X-Literal"), Some("$(printenv)"));
    let converted = CurlConverter::default()
        .call(
            "convert",
            json!({ "command": command, "language": "python" }),
        )
        .unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), converted["request"]);
    assert_eq!(
        serde_json::to_value(warnings).unwrap(),
        converted["warnings"]
    );
    assert_eq!(
        parse_command(&"x".repeat(MAX_INPUT + 1)).unwrap_err().code,
        "curl.too_large"
    );
    assert_eq!(parse_command("  ").unwrap_err().code, "curl.empty");
}

#[test]
fn public_option_parser_retains_ignored_settings_without_changing_results() {
    let command = "curl https://example.com -sSL --connect-timeout 2 --cacert /not/read -0";
    let (request, warnings, options) = parse_command_with_options(command).unwrap();
    let expected = parse_command(command).unwrap();
    assert_eq!((request, warnings), expected);
    assert_eq!(
        options,
        [
            "-s",
            "-S",
            "-0",
            "--location",
            "--connect-timeout",
            "--cacert"
        ]
    );
}
