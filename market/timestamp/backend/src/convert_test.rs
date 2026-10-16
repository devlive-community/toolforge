use super::*;
use serde_json::json;

fn convert(value: serde_json::Value) -> PluginResult<Converted> {
    from_timestamp(serde_json::from_value(value).unwrap())
}

#[test]
fn detects_units_by_digit_count() {
    assert_eq!(detect(10), Unit::S);
    assert_eq!(detect(13), Unit::Ms);
    assert_eq!(detect(16), Unit::Us);
    assert_eq!(detect(19), Unit::Ns);
}

#[test]
fn converts_seconds_with_calendar_details() {
    let out = convert(json!({"value": "1700000000", "timezones": ["Asia/Shanghai"]})).unwrap();
    assert_eq!(out.unit, Unit::S);
    assert_eq!(out.iso, "2023-11-14T22:13:20Z");
    assert_eq!(out.rfc2822, "Tue, 14 Nov 2023 22:13:20 +0000");
    assert_eq!((out.weekday, out.day_of_year, out.iso_week), (2, 318, 46));
    let shanghai = out
        .rows
        .iter()
        .find(|r| r.timezone == "Asia/Shanghai")
        .unwrap();
    assert_eq!(shanghai.datetime, "2023-11-15 06:13:20.000");
    assert_eq!(shanghai.offset, "+08:00");
    let utc = out.rows.iter().find(|r| r.timezone == "UTC").unwrap();
    assert_eq!(utc.datetime, "2023-11-14 22:13:20.000");
}

#[test]
fn auto_detects_milliseconds_and_nanoseconds() {
    let ms = convert(json!({"value": "1700000000123"})).unwrap();
    assert_eq!(ms.unit, Unit::Ms);
    assert_eq!(ms.milliseconds, 1_700_000_000_123);
    let ns = convert(json!({"value": "1700000000123456789"})).unwrap();
    assert_eq!(ns.unit, Unit::Ns);
    assert_eq!(ns.seconds, 1_700_000_000);
}

#[test]
fn explicit_unit_and_separators() {
    let out = convert(json!({"value": "1_700_000_000_000", "unit": "ms"})).unwrap();
    assert_eq!(out.seconds, 1_700_000_000);
}

#[test]
fn negative_timestamps_are_before_epoch() {
    let out = convert(json!({"value": "-86400"})).unwrap();
    assert_eq!(out.iso, "1969-12-31T00:00:00Z");
}

#[test]
fn local_and_utc_rows_come_first_without_duplicates() {
    let out = convert(json!({"value": "0", "timezones": ["UTC", "Asia/Tokyo"]})).unwrap();
    assert_eq!(out.rows.len(), 3);
    assert_eq!(out.rows[1].timezone, "UTC");
    assert_eq!(out.rows[2].timezone, "Asia/Tokyo");
}

#[test]
fn rejects_bad_input() {
    assert_eq!(
        convert(json!({"value": ""})).unwrap_err().code,
        "time.empty"
    );
    assert_eq!(
        convert(json!({"value": "12ab"})).unwrap_err().code,
        "time.not_a_number"
    );
    assert_eq!(
        convert(json!({"value": "99999999999999999", "unit": "s"}))
            .unwrap_err()
            .code,
        "time.out_of_range"
    );
    assert_eq!(
        convert(json!({"value": "1", "timezones": ["Mars/Base"]}))
            .unwrap_err()
            .code,
        "time.invalid_timezone"
    );
}

#[test]
fn now_is_close_to_system_time() {
    let now = now();
    let expected = Timestamp::now().as_second();
    assert!((now.seconds - expected).abs() <= 1);
    assert_eq!(now.milliseconds / 1000, now.seconds);
}
