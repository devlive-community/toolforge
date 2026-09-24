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
