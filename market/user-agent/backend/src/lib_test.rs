use super::*;
use serde_json::json;

#[test]
fn dispatches_and_detects() {
    let tool = UserAgent::default();
    let out = tool.call("parse", json!({ "ua": "curl/8.7.1" })).unwrap();
    assert_eq!(out["bot"]["name"], "curl");
    assert_eq!(out["device"]["kind"], "bot");
    let out = tool
        .call("analyze", json!({ "input": "curl/8.7.1\ncurl/8.7.1" }))
        .unwrap();
    assert_eq!(out["stats"]["unique"], 1);
    assert_eq!(out["rows"][0]["count"], 2);
    assert!(tool.detect("Wget/1.21.4").is_some());
    assert!(tool.detect("hello").is_none());
    for function in ["parse", "analyze", "export"] {
        assert!(tool.manifest().functions.contains_key(function));
    }
}
