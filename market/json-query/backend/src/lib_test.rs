use serde_json::json;

use super::*;

#[test]
fn answers_queries_in_both_languages() {
    let tool = JsonQuery::default();
    let jq = tool
        .call(
            "query",
            json!({ "source": { "text": "[1,2,3]" }, "query": "map(. * 2)", "compact": true }),
        )
        .unwrap();
    assert_eq!(jq["results"][0]["text"], "[2,4,6]");
    assert_eq!(jq["inputs"], 1);
    assert_eq!(jq["truncated"], false);
    let path = tool
        .call(
            "query",
            json!({ "source": { "text": "[1,2,3]" }, "query": "$[1]", "language": "jsonpath" }),
        )
        .unwrap();
    assert_eq!(path["results"][0], json!({ "path": "$[1]", "text": "2" }));
    let bad = tool
        .call(
            "query",
            json!({ "source": { "text": "[1," }, "query": "." }),
        )
        .unwrap_err();
    assert_eq!(bad.code, "query.invalid_json");
}

#[test]
fn accepts_flat_input_and_path() {
    let plugin = JsonQuery::default();
    let text = plugin
        .call("query", json!({ "input": "{\"a\":1}", "query": ".a" }))
        .unwrap();
    assert_eq!(text["results"][0]["text"], "1");

    let file = std::env::temp_dir().join(format!("tf-json-query-{}.json", std::process::id()));
    std::fs::write(&file, "[\"x\"]").unwrap();
    let from_file = plugin
        .call(
            "query",
            json!({ "path": file, "query": ".[0]", "raw": true }),
        )
        .unwrap();
    std::fs::remove_file(&file).unwrap();
    assert_eq!(from_file["results"][0]["text"], "x");
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&JsonQuery::default()).unwrap();
    assert_eq!(checked, 0);
}
