use super::*;
use serde_json::json;

#[test]
fn plan_roundtrips_via_json() {
    let out = plan_fn(json!({
        "members": [{"id": "a", "name": "爸爸"}, {"id": "b", "name": "妈妈"}],
        "chores": [{
            "id": "c1",
            "name": "倒垃圾",
            "epochs": [{"from": "2026-10-01", "rule": {"kind": "daily"}, "mode": {"type": "rotation", "memberIds": ["a", "b"]}}]
        }],
        "records": {},
        "today": "2026-10-05",
        "fromDate": "2026-10-05",
        "toDate": "2026-10-06"
    }))
    .unwrap();
    assert_eq!(out["occurrences"].as_array().unwrap().len(), 2);
    assert_eq!(out["occurrences"][0]["date"], json!("2026-10-05"));
    assert_eq!(out["occurrences"][0]["assigneeName"], json!("爸爸"));
    assert_eq!(out["occurrences"][0]["status"], json!("pending"));
    assert_eq!(out["stats"].as_array().unwrap().len(), 2);
}

#[test]
fn plan_rejects_unknown_shape() {
    let error = plan_fn(json!({ "members": [] })).unwrap_err();
    assert!(!error.code.is_empty());
}

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    let plugin = Chores::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}

#[test]
fn declared_interfaces_are_accepted_by_backend() {
    let plugin = Chores::default();
    let checked = tf_plugin_api::interface::check_declared_interfaces(&plugin)
        .expect("declared interfaces must match the backend");
    assert!(checked >= 1);
}
