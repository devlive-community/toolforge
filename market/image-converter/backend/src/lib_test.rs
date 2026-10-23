use super::*;
use serde_json::json;

#[test]
fn convert_is_a_task_only_function() {
    let tool = ImageConverter::default();
    assert!(tool.manifest().functions["convert"].task);
    assert_eq!(
        tool.call("convert", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&ImageConverter::default()).unwrap();
    assert_eq!(checked, 0);
}
