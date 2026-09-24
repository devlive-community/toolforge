use super::*;
use serde_json::json;

#[test]
fn dispatches_snapshot() {
    let out = SystemMonitor::default()
        .call("snapshot", json!({}))
        .unwrap();
    assert!(out["cpu"]["logicalCores"].as_u64().unwrap() > 0);
    assert!(out["processes"].is_null());
}
