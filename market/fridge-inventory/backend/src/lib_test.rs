use super::*;
use serde_json::json;

fn sample_batch() -> serde_json::Value {
    json!({
        "id": "b1",
        "name": "酸奶",
        "location": "fridge",
        "unit": "pack",
        "intake": 3.0,
        "purchasedDate": "2026-10-01",
        "labelDate": "2026-10-12"
    })
}

#[test]
fn review_roundtrips_via_json() {
    let out = review_fn(json!({
        "batches": [sample_batch()],
        "movements": [
            {"id": "m1", "batchId": "b1", "kind": "consume", "amount": 1.0, "date": "2026-10-05"}
        ],
        "today": "2026-10-10",
        "soonDays": 3
    }))
    .unwrap();
    let batches = out["batches"].as_array().unwrap();
    assert_eq!(batches.len(), 1);
    assert!((batches[0]["remaining"].as_f64().unwrap() - 2.0).abs() < 1e-12);
    assert_eq!(batches[0]["labelState"], json!("soon"));
    assert_eq!(batches[0]["status"], json!("active"));
}

#[test]
fn apply_roundtrips_via_json() {
    let out = apply_fn(json!({
        "batches": [sample_batch()],
        "movements": [],
        "op": {
            "type": "deduct",
            "movementId": "m9",
            "batchId": "b1",
            "kind": "consume",
            "amount": 1.0,
            "unit": "pack",
            "date": "2026-10-10"
        },
        "today": "2026-10-10"
    }))
    .unwrap();
    assert_eq!(out["movement"]["id"], json!("m9"));
    assert!((out["remaining"].as_f64().unwrap() - 2.0).abs() < 1e-12);
}

#[test]
fn rejects_malformed_args() {
    let error = review_fn(json!({ "batches": [] })).unwrap_err();
    assert!(!error.code.is_empty());
    let error = apply_fn(json!({ "batches": [], "movements": [] })).unwrap_err();
    assert!(!error.code.is_empty());
}
