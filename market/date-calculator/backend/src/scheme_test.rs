use super::*;

fn d(y: i16, m: i8, day: i8) -> Date {
    Date::new(y, m, day).unwrap()
}

fn day(date: &str, work: bool, name: Option<&str>) -> CustomDay {
    CustomDay {
        date: date.to_owned(),
        work,
        name: name.map(str::to_owned),
    }
}

fn scheme(base: Base, weekend: Vec<u8>, days: Vec<CustomDay>) -> Scheme {
    Scheme {
        name: "Team".to_owned(),
        base,
        weekend,
        days,
    }
}

#[test]
fn overrides_take_precedence_over_legal_holidays_and_weekends() {
    let rules = Rules::from_scheme(&scheme(
        Base::China,
        vec![6, 7],
        vec![
            // 公司年会：周三休息
            day("2025-03-05", false, Some("Annual party")),
            // 国庆假期中加班
            day("2025-10-02", true, None),
        ],
    ))
    .unwrap();
    assert_eq!(rules.day(d(2025, 3, 5)), (false, Source::Custom));
    assert_eq!(rules.day(d(2025, 10, 2)), (true, Source::Custom));
    assert_eq!(rules.day(d(2025, 10, 3)), (false, Source::Legal));
    // 法定调休上班的周六
    assert_eq!(rules.day(d(2025, 9, 28)), (true, Source::Legal));
    assert_eq!(rules.day(d(2025, 3, 8)), (false, Source::Weekend));
    assert_eq!(rules.day(d(2025, 3, 6)), (true, Source::Weekday));
    assert_eq!(
        rules.custom(d(2025, 3, 5)).and_then(|c| c.name.as_deref()),
        Some("Annual party")
    );
}

#[test]
fn custom_weekends_without_legal_holidays() {
    // 周五周六为周末的地区
    let rules = Rules::from_scheme(&scheme(Base::Weekdays, vec![5, 6], vec![])).unwrap();
    assert!(!rules.is_workday(d(2025, 10, 3)));
    assert!(!rules.is_workday(d(2025, 10, 4)));
    assert!(rules.is_workday(d(2025, 10, 5)));
    // 不依赖法定数据
    assert!(rules.is_workday(d(2025, 10, 1)));
    assert!(!rules.beyond_data(d(2099, 1, 1)));
    assert!(Rules::builtin(Base::China).beyond_data(d(2099, 1, 1)));
}

#[test]
fn normalizes_and_validates() {
    let normalized = normalize(scheme(
        Base::China,
        vec![7, 6, 6],
        vec![
            day("2025-05-02", false, Some("  ")),
            day("2025-01-01", true, Some(" A ")),
            day("2025-05-02", true, None),
        ],
    ))
    .unwrap();
    assert_eq!(normalized.weekend, [6, 7]);
    assert_eq!(
        normalized.days,
        [
            day("2025-01-01", true, Some("A")),
            day("2025-05-02", true, None)
        ]
    );

    let mut nameless = scheme(Base::China, vec![], vec![]);
    nameless.name = " ".into();
    assert_eq!(
        normalize(nameless).unwrap_err().code,
        "scheme.name_required"
    );
    assert_eq!(
        normalize(scheme(Base::China, vec![8], vec![]))
            .unwrap_err()
            .code,
        "scheme.invalid_weekday"
    );
    assert_eq!(
        normalize(scheme(
            Base::China,
            vec![],
            vec![day("2025-02-30", true, None)]
        ))
        .unwrap_err()
        .code,
        "scheme.invalid_date"
    );
}

#[test]
fn round_trips_through_export_and_import() {
    let original = scheme(
        Base::Weekdays,
        vec![5, 6],
        vec![day("2025-12-25", false, Some("Christmas"))],
    );
    let text = export(original.clone()).unwrap();
    assert!(text.contains("\"format\": \"toolforge-holiday-scheme\""));
    assert_eq!(import(&text).unwrap(), original);
    // 只有方案字段的 JSON 也能导入，缺省字段取默认值
    let minimal = import(r#"{"name":"Minimal"}"#).unwrap();
    assert_eq!((minimal.base, minimal.weekend), (Base::China, vec![6, 7]));
    assert_eq!(import("not json").unwrap_err().code, "scheme.invalid_json");
}

#[test]
fn calendar_argument_accepts_names_or_schemes() {
    let named: CalendarArg = serde_json::from_str("\"weekdays\"").unwrap();
    assert!(named.rules().unwrap().is_workday(d(2025, 10, 1)));
    let custom: CalendarArg =
        serde_json::from_str(r#"{"name":"X","base":"weekdays","weekend":[7],"days":[]}"#).unwrap();
    assert!(custom.rules().unwrap().is_workday(d(2025, 10, 4)));
}
