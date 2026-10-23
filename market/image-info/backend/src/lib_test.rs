use super::*;
use serde_json::json;

#[test]
fn unknown_function_is_rejected() {
    let tool = ImageInfo::default();
    assert!(tool.manifest().functions.contains_key("strip_metadata"));
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}
