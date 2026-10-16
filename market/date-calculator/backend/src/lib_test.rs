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

#[test]
fn accepts_holiday_schemes() {
    let scheme = json!({
        "name": "Weekend Fri-Sat",
        "base": "weekdays",
        "weekend": [5, 6],
        "days": [{ "date": "2025-10-07", "work": false, "name": "Local" }]
    });
    let work = call(
        "add_workdays",
        json!({ "start": "2025-10-02", "days": 3, "calendar": scheme }),
    )
    .unwrap();
    // 10-03 周五、10-04 周六为周末，10-07 自定义休息
    assert_eq!(work["result"], "2025-10-08");
    let diff = call(
        "diff",
        json!({ "start": "2025-10-01", "end": "2025-10-31", "includeEnd": true, "scheme": scheme }),
    )
    .unwrap();
    assert_eq!(diff["workdays"]["custom"]["restDays"], 1);
    let month = call(
        "month",
        json!({ "year": 2025, "month": 10, "scheme": scheme }),
    )
    .unwrap();
    assert!(month[0]["workday"].is_boolean());

    let text = call("scheme_export", json!({ "scheme": scheme })).unwrap();
    let imported = call("scheme_import", json!({ "text": text })).unwrap();
    assert_eq!(imported["weekend"], json!([5, 6]));
    assert_eq!(imported["days"][0]["name"], "Local");
    for function in ["scheme_import", "scheme_export"] {
        assert!(
            DateCalculator::default()
                .manifest()
                .functions
                .contains_key(function)
        );
    }
}
