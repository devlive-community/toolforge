use super::*;
use crate::parse::moment;

fn d(y: i16, m: i8, day: i8) -> Date {
    Date::new(y, m, day).unwrap()
}

#[test]
fn counts_days_and_calendar_spans() {
    let diff = difference(
        moment("2024-01-31").unwrap(),
        moment("2025-03-01").unwrap(),
        false,
    )
    .unwrap();
    assert_eq!((diff.years, diff.months, diff.days), (1, 1, 1));
    assert_eq!(diff.total_days, 395);
    assert_eq!((diff.weeks, diff.week_days), (56, 3));
    assert_eq!(diff.total_hours, 395 * 24);
    assert!(!diff.negative);
    let back = difference(
        moment("2025-03-01").unwrap(),
        moment("2024-01-31").unwrap(),
        false,
    )
    .unwrap();
    assert!(back.negative);
    assert_eq!(back.total_days, 395);
    // 包含结束日多算一天
    let inclusive = difference(
        moment("2025-01-01").unwrap(),
        moment("2025-01-31").unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(inclusive.total_days, 31);
    assert_eq!((inclusive.months, inclusive.days), (1, 0));
}

#[test]
fn includes_times_when_given() {
    let diff = difference(
        moment("2025-01-01 08:00").unwrap(),
        moment("2025-01-02 09:30:15").unwrap(),
        false,
    )
    .unwrap();
    assert!(diff.has_time);
    assert_eq!(
        (diff.days, diff.hours, diff.minutes, diff.seconds),
        (1, 1, 30, 15)
    );
    assert_eq!(diff.total_seconds, 86400 + 5415);
    assert_eq!(diff.total_minutes, (86400 + 5415) / 60);
    assert!(diff.workdays.is_none());
}

#[test]
fn counts_workdays_with_chinese_holidays() {
    // 2025 年 10 月：国庆中秋 1–8 日放假，9 月 28 日与 10 月 11 日调休上班
    let october = count_workdays(d(2025, 10, 1), d(2025, 10, 31)).unwrap();
    assert_eq!(october.weekdays, 23);
    assert_eq!(october.holidays, 6);
    assert_eq!(october.adjusted, 1);
    assert_eq!(october.china, 18);
    assert!(!october.beyond_data);
    let late = count_workdays(d(2030, 1, 1), d(2030, 1, 7)).unwrap();
    assert!(late.beyond_data);
    assert_eq!(late.china, late.weekdays);
    // 起止顺序无关
    assert_eq!(
        count_workdays(d(2025, 10, 31), d(2025, 10, 1)).unwrap(),
        october
    );
    let diff = difference(
        moment("2025-10-01").unwrap(),
        moment("2025-10-31").unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(diff.workdays.unwrap().china, 18);
    let exclusive = difference(
        moment("2025-10-01").unwrap(),
        moment("2025-10-31").unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(exclusive.workdays.unwrap().weekdays, 22);
}

#[test]
fn adds_months_with_clamping() {
    let jan31 = moment("2024-01-31").unwrap();
    let added = add(
        jan31,
        Amount {
            months: 1,
            ..Amount::default()
        },
    )
    .unwrap();
    assert_eq!(added.result, "2024-02-29");
    assert!(added.clamped);
    assert_eq!(added.weekday, 3);
    let back = add(
        jan31,
        Amount {
            years: -1,
            days: 10,
            ..Amount::default()
        },
    )
    .unwrap();
    assert_eq!(back.result, "2023-02-10");
    assert!(!back.clamped);
    let timed = add(
        moment("2025-01-01").unwrap(),
        Amount {
            hours: 36,
            minutes: 5,
            ..Amount::default()
        },
    )
    .unwrap();
    assert_eq!(timed.result, "2025-01-02 12:05:00");
    assert_eq!(
        add(
            jan31,
            Amount {
                years: 20_000,
                ..Amount::default()
            }
        )
        .unwrap_err()
        .code,
        "date.out_of_range"
    );
}

#[test]
fn adds_working_days() {
    // 2025-09-30（周二）之后的第 1 个法定工作日是 10 月 9 日
    let next = add_workdays(d(2025, 9, 30), 1, Calendar::China).unwrap();
    assert_eq!(next.result, "2025-10-09");
    assert_eq!(
        add_workdays(d(2025, 9, 30), 1, Calendar::Weekdays)
            .unwrap()
            .result,
        "2025-10-01"
    );
    // 往前数：10 月 13 日（周一）之前第 1 个工作日是调休的 10 月 11 日（周六）
    assert_eq!(
        add_workdays(d(2025, 10, 13), -1, Calendar::China)
            .unwrap()
            .result,
        "2025-10-11"
    );
    assert_eq!(
        add_workdays(d(2025, 10, 13), 0, Calendar::China)
            .unwrap()
            .result,
        "2025-10-13"
    );
}
