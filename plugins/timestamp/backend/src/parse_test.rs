use super::*;

fn parse_at(input: &str, timezone: &str) -> PluginResult<Parsed> {
    to_timestamp(Args {
        input: input.into(),
        timezone: timezone.into(),
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
        parse_at("yesterday", "UTC").unwrap_err().code,
        "time.unrecognized"
    );
    assert_eq!(parse_at("  ", "UTC").unwrap_err().code, "time.empty");
    assert_eq!(
        parse_at("2024-02-30", "UTC").unwrap_err().code,
        "time.unrecognized"
    );
}
