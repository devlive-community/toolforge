use super::*;

fn d(y: i16, m: i8, day: i8) -> Date {
    Date::new(y, m, day).unwrap()
}

#[test]
fn knows_rest_days_and_make_up_workdays() {
    // 2025 年春节：1 月 28 日起放假，1 月 26 日（周日）与 2 月 8 日（周六）调休上班
    assert_eq!(
        lookup(d(2025, 1, 29)),
        Some(Holiday {
            index: 1,
            work: false
        })
    );
    assert_eq!(
        lookup(d(2025, 1, 26)),
        Some(Holiday {
            index: 1,
            work: true
        })
    );
    assert!(is_workday(d(2025, 2, 8), Calendar::China));
    assert!(!is_workday(d(2025, 2, 8), Calendar::Weekdays));
    assert!(!is_workday(d(2025, 1, 29), Calendar::China));
    assert!(is_workday(d(2025, 1, 29), Calendar::Weekdays));
    assert_eq!(lookup(d(2025, 3, 3)), None);
    assert!(last_year() >= 2026);
}
