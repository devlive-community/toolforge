use super::*;

#[test]
fn roundtrips_known_dates() {
    for (text, days) in [
        ("1970-01-01", 0),
        ("1970-01-02", 1),
        ("2000-02-29", 11016),
        ("2026-10-20", 20746),
        ("1971-12-31", 729),
    ] {
        assert_eq!(parse_date(text), Some(days), "{text}");
        assert_eq!(format_date(days), text);
    }
}

#[test]
fn rejects_pseudo_dates() {
    for text in [
        "2026-2-30",
        "2026-02-30",
        "2026-13-01",
        "2026-00-10",
        "2026-02-00",
        "2026/02/10",
        "20260210",
        "abcd-02-10",
        "",
        "2026-02-1a",
    ] {
        assert_eq!(parse_date(text), None, "{text} should be rejected");
    }
}

#[test]
fn add_days_crosses_months_and_years() {
    let base = parse_date("2026-10-31").unwrap();
    assert_eq!(format_date(base + 1), "2026-11-01");
    let leap = parse_date("2028-02-28").unwrap();
    assert_eq!(format_date(leap + 1), "2028-02-29");
    let year_end = parse_date("2026-12-31").unwrap();
    assert_eq!(format_date(year_end + 1), "2027-01-01");
}
