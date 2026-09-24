use super::*;
use serde_json::json;

#[test]
fn dispatches_functions() {
    let tool = IpCalculator::default();
    let out = tool
        .call("calculate", json!({"input": "10.0.0.1/8"}))
        .unwrap();
    assert_eq!(out["network"], "10.0.0.0");
    let range = tool
        .call(
            "range_to_cidrs",
            json!({"start": "10.0.0.0", "end": "10.0.0.255"}),
        )
        .unwrap();
    assert_eq!(range["cidrs"], json!(["10.0.0.0/24"]));
}
