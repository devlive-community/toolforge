use super::*;

#[test]
fn detects_timestamps_in_range() {
    let d = detect("1700000000").unwrap();
    assert_eq!((d.score, d.label.as_str()), (85, "timestamp"));
    assert_eq!(d.params["utc"], "2023-11-14 22:13:20 UTC");
    assert_eq!(detect("1700000000123").unwrap().params["unit"], "ms");
    assert!(detect("123456789").is_none(), "1973 is out of range");
    assert!(detect("99999999999").is_none());
    assert!(detect("12345").is_none());
}

#[test]
fn detects_dates() {
    assert_eq!(detect("2024-02-29 12:00:00").unwrap().label, "date");
    assert_eq!(detect("2024-02-29T12:00:00Z").unwrap().label, "date");
    assert_eq!(detect("2024-02-29").unwrap().label, "date");
    assert!(detect("2024-02-30").is_none());
    assert!(detect("hello world").is_none());
}
