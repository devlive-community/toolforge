use super::*;

#[test]
fn parses_plain_numbers_only() {
    for (text, value) in [
        ("1", 1.0),
        (" -2.5 ", -2.5),
        ("+3", 3.0),
        ("1e3", 1000.0),
        ("12.5%", 12.5),
        (".5", 0.5),
    ] {
        assert_eq!(number(text), Some(value), "{text}");
    }
    for text in ["", "abc", "inf", "NaN", "1,000", "12ms", "0x10", "e", "-"] {
        assert_eq!(number(text), None, "{text}");
    }
}

#[test]
fn parses_datetimes_in_common_forms() {
    let utc = datetime("2024-01-02T03:04:05Z").unwrap();
    assert_eq!(utc, 1_704_164_645_000.0);
    assert_eq!(datetime("2024-01-02T11:04:05+08:00"), Some(utc));
    assert_eq!(datetime("2024-01-02T03:04:05.250Z"), Some(utc + 250.0));
    // 不带偏移的按本地时间：与同一写法的本地解释一致
    let local = datetime("2024-01-02 03:04:05").unwrap();
    assert_eq!(datetime("2024/01/02 03:04:05"), Some(local));
    assert_eq!(datetime("2024-01-02T03:04:05"), Some(local));
    assert_eq!(
        datetime("2024-01-02").unwrap() + (3 * 3600 + 4 * 60 + 5) as f64 * 1000.0,
        local
    );
    for text in ["20240102", "12:00:00", "2024", "hello", "1700000000"] {
        assert_eq!(datetime(text), None, "{text}");
    }
}

#[test]
fn treats_common_placeholders_as_missing() {
    for text in ["", " ", "-", "null", "NaN", "N/A", "None"] {
        assert!(is_missing(text), "{text:?}");
    }
    assert!(!is_missing("0"));
}

#[test]
fn infers_columns_with_tolerance() {
    let raw = |cells: &[&str]| cells.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>();
    // 90% 以上能解析才算数值
    let mut cells: Vec<&str> = vec!["1"; 9];
    cells.push("oops");
    assert_eq!(column("v", &raw(&cells)).0, Kind::Number);
    cells.push("bad");
    assert_eq!(column("v", &raw(&cells)).0, Kind::Text);
    assert_eq!(column("v", &raw(&["", "null"])).0, Kind::Text);
    // 列名像时间且数值像时间戳才转成时间
    assert_eq!(column("created_at", &raw(&["1700000000123"])).0, Kind::Time);
    assert_eq!(column("count", &raw(&["1700000000"])).0, Kind::Number);
    assert_eq!(column("time", &raw(&["5", "6"])).0, Kind::Number);
    let (kind, values) = column("时间", &raw(&["1700000000", ""]));
    assert_eq!(
        (kind, values),
        (Kind::Time, vec![Some(1_700_000_000_000.0), None])
    );
}
