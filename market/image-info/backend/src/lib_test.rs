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

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&ImageInfo::default()).unwrap();
    assert_eq!(checked, 0);
}
