use super::*;

fn parse_at(input: &str, timezone: &str) -> PluginResult<Parsed> {
    to_timestamp(Args {
        input: input.into(),
        timezone: timezone.into(),
        choice: 0,
    })
}

#[test]
fn parses_offset_formats_regardless_of_selected_zone() {
    for input in [
        "2023-11-14T22:13:20Z",
        "2023-11-15T06:13:20+08:00",
        "2023-11-15T06:13:20+08:00[Asia/Shanghai]",
        "Tue, 14 Nov 2023 22:13:20 +0000",
    ] {
        assert_eq!(
            parse_at(input, "Asia/Tokyo").unwrap().seconds,
            1_700_000_000,
            "{input}"
        );
    }
}

#[test]
fn naive_datetime_uses_selected_timezone() {
    let out = parse_at("2024-02-29 12:00:00", "Asia/Shanghai").unwrap();
    assert_eq!(out.seconds, 1_709_179_200);
    assert_eq!(out.format, "datetime");
    assert_eq!(out.zoned.offset, "+08:00");
    assert_eq!(
        parse_at("2024-02-29 12:00:00", "UTC").unwrap().seconds,
        1_709_208_000
    );
}

#[test]
fn accepts_slashes_and_single_digit_parts() {
    let out = parse_at("2024/2/29", "UTC").unwrap();
    assert_eq!(out.seconds, 1_709_164_800);
    assert_eq!(out.format, "date");
    assert_eq!(
        parse_at("2024.02.29 08:30", "UTC").unwrap().seconds,
        1_709_195_400
    );
}

#[test]
fn keeps_sub_second_precision() {
    let out = parse_at("2023-11-14T22:13:20.123456789Z", "UTC").unwrap();
    assert_eq!(out.milliseconds, 1_700_000_000_123);
    assert_eq!(out.microseconds, 1_700_000_000_123_456);
    assert_eq!(out.nanoseconds, "1700000000123456789");
}

#[test]
fn rejects_unrecognized_input() {
    assert_eq!(
        parse_at("not a date", "UTC").unwrap_err().code,
        "time.unrecognized"
    );
    assert_eq!(parse_at("  ", "UTC").unwrap_err().code, "time.empty");
    assert_eq!(
        parse_at("2024-02-30", "UTC").unwrap_err().code,
        "time.unrecognized"
    );
}

#[test]
fn natural_language_lists_candidates_and_honors_the_choice() {
    let now: Zoned = "2025-10-15T10:20:30+08:00[Asia/Shanghai]".parse().unwrap();
    let args = |choice| Args {
        input: "下周五 9点".into(),
        timezone: "Asia/Shanghai".into(),
        choice,
    };
    let first = to_timestamp_at(args(0), Some(&now)).unwrap();
    assert_eq!(first.format, "natural");
    assert_eq!(first.candidates.len(), 4);
    assert_eq!(first.zoned.datetime, "2025-10-24 09:00:00.000");
    assert_eq!(first.candidates[0].weekday, 5);
    assert_eq!(first.candidates[0].notes, ["nextWeek", "am"]);
    let third = to_timestamp_at(args(2), Some(&now)).unwrap();
    assert_eq!(third.choice, 2);
    assert_eq!(third.zoned.datetime, "2025-10-17 09:00:00.000");
    // 越界的选择收在最后一个
    assert_eq!(to_timestamp_at(args(99), Some(&now)).unwrap().choice, 3);
    // 精确格式没有候选
    let exact = parse_at("2024-02-29 12:00:00", "UTC").unwrap();
    assert!(exact.candidates.is_empty());
}
