use super::*;

fn call(function: &str, args: Value) -> Value {
    RandomTool::default().call(function, args).unwrap()
}

#[test]
fn every_operation_reports_seed_usage() {
    let n = call(
        "numbers",
        json!({ "min": 1, "max": 10, "count": 3, "seed": "x" }),
    );
    assert_eq!(n["seeded"], true);
    assert_eq!(n["algorithm"], "toolforge-random-v1");
    assert_eq!(
        n,
        call(
            "numbers",
            json!({ "min": 1, "max": 10, "count": 3, "seed": "x" })
        )
    );
    let d = call(
        "draw",
        json!({ "items": "Ann\nBob*2", "weighted": true, "count": 2 }),
    );
    assert_eq!(
        (d["seeded"].as_bool(), d["total"].as_u64()),
        (Some(false), Some(2))
    );
    let s = call("shuffle", json!({ "items": "a\nb\nc" }));
    assert_eq!(s["items"].as_array().unwrap().len(), 3);
    let g = call("groups", json!({ "items": "a\nb\nc\nd", "groups": 2 }));
    assert_eq!(g["groups"].as_array().unwrap().len(), 2);
    let r = call("dice", json!({ "count": 3, "sides": 6, "seed": "x" }));
    let sum: u64 = r["rolls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .sum();
    assert_eq!(r["sum"].as_u64(), Some(sum));
}
