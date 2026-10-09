use super::*;

fn call(function: &str, args: Value) -> PluginResult<Value> {
    DateCalculator::default().call(function, args)
}

#[test]
fn exposes_every_function() {
    let today = call("today", json!({})).unwrap();
    assert!(today["holidayDataUntil"].as_i64().unwrap() >= 2026);
    let diff = call(
        "diff",
        json!({ "start": "2025-01-01", "end": "2025-12-31", "includeEnd": true }),
    )
    .unwrap();
    assert_eq!(diff["totalDays"], 365);
    assert!(diff["workdays"]["china"].as_i64().unwrap() > 240);
    let added = call("add", json!({ "start": "2025-01-31", "months": 1 })).unwrap();
    assert_eq!(added["result"], "2025-02-28");
    let work = call(
        "add_workdays",
        json!({ "start": "2025-09-30", "days": 1, "calendar": "china" }),
    )
    .unwrap();
    assert_eq!(work["result"], "2025-10-09");
    let info = call("info", json!({ "date": "2025-10-06" })).unwrap();
    assert_eq!(info["holiday"]["index"], 7);
    let month = call("month", json!({ "year": 2025, "month": 2 })).unwrap();
    assert_eq!(month.as_array().unwrap().len(), 42);
    assert_eq!(
        call("diff", json!({ "start": "nope", "end": "2025-01-01" }))
            .unwrap_err()
            .code,
        "date.invalid"
    );
}
