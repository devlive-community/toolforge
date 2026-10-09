use super::*;

fn d(y: i16, m: i8, day: i8) -> Date {
    Date::new(y, m, day).unwrap()
}

#[test]
fn describes_a_day() {
    let info = info(d(2025, 1, 29)).unwrap();
    assert_eq!(info.weekday, 2);
    assert_eq!((info.iso_year, info.iso_week), (2025, 5));
    assert_eq!(info.day_of_year, 29);
    assert_eq!(info.days_in_year, 365);
    assert_eq!(info.quarter, 1);
    let lunar = info.lunar.unwrap();
    assert_eq!(lunar.text, "农历乙巳年正月初一");
    assert_eq!((lunar.month.as_str(), lunar.day.as_str()), ("正月", "初一"));
    assert_eq!(lunar.zodiac, 5, "snake");
    assert_eq!(info.festivals, ["春节"]);
    assert_eq!(
        info.holiday,
        Some(Holiday {
            index: 1,
            work: false
        })
    );
    assert_eq!(info.constellation, 10, "aquarius");
    let term = info.term.unwrap();
    assert_eq!((term.index, term.day), (2, 9));
}

#[test]
fn handles_leap_months_and_terms() {
    // 2023 年闰二月从 3 月 22 日开始
    let leap = info(d(2023, 3, 22)).unwrap().lunar.unwrap();
    assert!(leap.leap);
    assert_eq!(leap.day, "初一");
    let qingming = info(d(2024, 4, 4)).unwrap();
    assert_eq!(qingming.term.unwrap(), Term { index: 7, day: 0 });
    // 农历范围之外只给公历信息
    let far = info(d(2200, 1, 1)).unwrap();
    assert!(far.lunar.is_none() && far.term.is_none());
}

#[test]
fn builds_month_grids() {
    let cells = month(2025, 10, true).unwrap();
    assert_eq!(cells.len(), 42);
    // 2025-10-01 是周三，周一开头时前面有两天上个月的日子
    assert_eq!(cells[0].date, "2025-09-29");
    assert!(!cells[0].in_month);
    let first = &cells[2];
    assert_eq!(first.date, "2025-10-01");
    assert_eq!(
        first.holiday,
        Some(Holiday {
            index: 7,
            work: false
        })
    );
    let mid_autumn = cells.iter().find(|c| c.date == "2025-10-06").unwrap();
    assert_eq!(mid_autumn.label, "中秋节");
    assert!(mid_autumn.special);
    let cold_dew = cells.iter().find(|c| c.date == "2025-10-08").unwrap();
    assert_eq!(cold_dew.label, "寒露");
    let lunar_first = cells.iter().find(|c| c.date == "2025-10-21").unwrap();
    assert_eq!(lunar_first.label, "九月");
    let sunday_first = month(2025, 10, false).unwrap();
    assert_eq!(sunday_first[0].date, "2025-09-28");
}
