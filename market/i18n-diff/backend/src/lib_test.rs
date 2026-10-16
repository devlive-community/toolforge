use super::*;
use serde_json::json;

#[test]
fn dispatches_functions() {
    let tool = I18nDiff::default();
    let files = json!([
        { "name": "en.json", "text": "{\"a\":\"A\",\"b\":\"B\"}" },
        { "name": "zh.json", "text": "{\"a\":\"甲\"}" }
    ]);
    let out = tool.call("compare", json!({ "files": files })).unwrap();
    assert_eq!(out["files"][1]["missing"], 1);
    let csv = tool
        .call("report", json!({ "files": files, "format": "csv" }))
        .unwrap();
    assert_eq!(
        csv,
        "file,key,issue,base,value,missing_placeholders,extra_placeholders\nzh.json,b,missing,B,,,\n"
    );
    let json_report = tool
        .call("report", json!({ "files": files, "format": "json" }))
        .unwrap();
    assert!(json_report.as_str().unwrap().contains("\"missing\""));
    let written = tool
        .call(
            "write",
            json!({ "base": files[0], "target": files[1], "fill": "base" }),
        )
        .unwrap();
    assert_eq!(written["added"], json!(["b"]));
    for function in ["compare", "write", "report"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
