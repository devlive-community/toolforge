use super::*;
use serde_json::json;

#[test]
fn dispatches_declared_functions() {
    let tool = XmlFormatter::default();
    for function in ["format", "minify", "validate"] {
        assert!(tool.manifest().functions.contains_key(function));
        let out = tool
            .call(function, json!({"input": "<a><b/></a>"}))
            .unwrap();
        assert_eq!(out["stats"]["elements"], 2);
    }
}
