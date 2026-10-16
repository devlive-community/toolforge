use super::*;
use serde_json::json;

#[test]
fn dispatches_analyze() {
    let tool = Semver::default();
    let out = tool
        .call(
            "analyze",
            json!({"versions": "1.2.3\n2.0.0", "range": "^1", "includePrerelease": true}),
        )
        .unwrap();
    assert_eq!(out["matched"], 1);
    assert_eq!(out["rows"][0]["outcome"]["kind"], "match");
    assert!(tool.manifest().functions.contains_key("analyze"));
}
