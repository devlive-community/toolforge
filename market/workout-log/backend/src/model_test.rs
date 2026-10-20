use super::*;
#[test]
fn parses_real_counts_and_load_without_inventing_bodyweight() {
    assert_eq!(weight("").unwrap(), None);
    assert_eq!(weight("12.125").unwrap(), Some(12.125));
    for v in ["-1", "NaN", "inf", "1e3", ".1234", "10001"] {
        assert!(weight(v).is_err(), "{v}");
    }
    for v in ["-1", "2.5", "10001"] {
        assert!(repetitions(v).is_err());
    }
    let mut set = Set {
        reps: "8".into(),
        weight: "10".into(),
        status: Status::Done,
        ..Set::default()
    };
    assert!(validate_set(&set, Mode::Bodyweight).is_err());
    set.weight = String::new();
    assert!(validate_set(&set, Mode::Bodyweight).is_ok());
    assert!(validate_set(&set, Mode::External).is_err());
    set.unit = Unit::Lb;
    set.weight = "100".into();
    assert!((kilograms(&set).unwrap().unwrap() - 45.359237).abs() < 0.000001);
}
#[test]
fn dates_and_document_limits_are_explicit() {
    assert!(date("2024-02-29").is_ok());
    assert!(date("2025-02-29").is_err());
    assert!(date("2025-2-3").is_err());
    assert!(budget(&"x".repeat(MAX_DOCUMENT)).is_err());
    let mut head = Head::empty();
    head.version = 2;
    assert!(validate_head(&head).is_err());
}
