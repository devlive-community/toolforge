use super::*;
use serde_json::json;

#[test]
fn generates_and_reports_errors() {
    let tool = JsonToCode::default();
    let out = tool
        .call(
            "generate",
            json!({"input": "{\"a\": 1}", "language": "typescript", "root": "my data"}),
        )
        .unwrap();
    assert_eq!(out["code"], "export interface MyData {\n  a: number;\n}\n");
    assert_eq!(out["types"], 1);
    let err = tool
        .call(
            "generate",
            json!({"input": "{\n  \"a\": }", "language": "go"}),
        )
        .unwrap_err();
    assert_eq!(
        (err.code.as_str(), &err.params["line"]),
        ("code.invalid_json", &json!(2))
    );
    assert_eq!(
        tool.call("generate", json!({"input": " ", "language": "go"}))
            .unwrap_err()
            .code,
        "code.empty"
    );
    assert_eq!(tool.call("languages", json!({})).unwrap()[6], "csharp");
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&JsonToCode::default()).unwrap();
    assert_eq!(checked, 2);
}
