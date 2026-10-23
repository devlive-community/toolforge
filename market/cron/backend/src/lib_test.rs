use super::*;
use serde_json::json;

#[test]
fn dispatches_evaluate() {
    let out = CronTool::default()
        .call(
            "evaluate",
            json!({"expression": "@hourly", "timezone": "UTC", "count": 3}),
        )
        .unwrap();
    assert_eq!(out["normalized"], "0 * * * *");
    assert_eq!(out["next"].as_array().unwrap().len(), 3);
    assert_eq!(out["fields"][0]["parts"][0]["kind"], "value");
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&CronTool::default()).unwrap();
    assert_eq!(checked, 2);
}
