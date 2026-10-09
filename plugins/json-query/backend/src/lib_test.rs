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
