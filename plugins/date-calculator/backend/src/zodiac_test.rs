use super::*;

fn solar(date: &str) -> Birth {
    birth(BirthArgs::Solar { date: date.into() }).unwrap()
}

#[test]
fn zodiac_differs_between_lichun_and_spring_festival() {
    let b = solar("2024-02-05");
    assert_eq!(
        (b.lichun.as_str(), b.spring_festival.as_str()),
        ("2024-02-04", "2024-02-10")
    );
    // 立春后已是甲辰龙年，春节前农历仍是癸卯兔年
    assert_eq!(
        (b.zodiac_lichun, b.lichun_year_ganzhi.as_str()),
        (4, "甲辰")
    );
    assert_eq!(
        (b.zodiac_lunar_year, b.lunar_year_ganzhi.as_str()),
        (3, "癸卯")
    );
    let after = solar("2024-02-10");
    assert_eq!((after.zodiac_lichun, after.zodiac_lunar_year), (4, 4));
}

#[test]
fn constellations_follow_boundaries() {
    assert_eq!(solar("2000-01-01").constellation, 9);
    assert_eq!(solar("2000-03-21").constellation, 0);
    assert_eq!(solar("2000-03-20").constellation, 11);
    let table = ranges();
    assert_eq!(
        table[0],
        Range {
            index: 0,
            start: [3, 21],
            end: [4, 19]
        }
    );
    assert_eq!(
        table[9],
        Range {
            index: 9,
            start: [12, 22],
            end: [1, 19]
        }
    );
    assert_eq!(
        table[11],
        Range {
            index: 11,
            start: [2, 19],
            end: [3, 20]
        }
    );
}

#[test]
fn accepts_lunar_birthdays_including_leap_months() {
    let leap = birth(BirthArgs::Lunar(LunarInput {
        year: 2023,
        month: 2,
        day: 1,
        leap: true,
    }))
    .unwrap();
    assert_eq!(leap.date, "2023-03-22");
    let err = birth(BirthArgs::Lunar(LunarInput {
        year: 2024,
        month: 2,
        day: 1,
        leap: true,
    }))
    .unwrap_err();
    assert_eq!(err.code, "date.lunar_invalid");
    let err = birth(BirthArgs::Solar {
        date: "1800-01-01".into(),
    })
    .unwrap_err();
    assert_eq!(err.code, "date.out_of_range");
}

#[test]
fn anniversaries_adjust_missing_days_and_leap_months() {
    let (day, adjusted) = occurrence(
        2024,
        &LunarInput {
            year: 0,
            month: 12,
            day: 30,
            leap: false,
        },
    )
    .unwrap();
    assert_eq!((day.get_day(), adjusted), (29, Some("lastDay")));
    let (_, adjusted) = occurrence(
        2024,
        &LunarInput {
            year: 0,
            month: 2,
            day: 1,
            leap: true,
        },
    )
    .unwrap();
    assert_eq!(adjusted, Some("noLeap"));
    let (day, adjusted) = occurrence(
        2023,
        &LunarInput {
            year: 0,
            month: 2,
            day: 1,
            leap: true,
        },
    )
    .unwrap();
    assert_eq!((day.get_month(), adjusted), (-2, None));

    let list = anniversaries(AnniversaryArgs {
        items: vec![
            LunarInput {
                year: 0,
                month: 1,
                day: 1,
                leap: false,
            },
            LunarInput {
                year: 0,
                month: 8,
                day: 15,
                leap: false,
            },
        ],
    })
    .unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|u| u.days >= 0 && u.days <= 400));
    assert!(list[0].lunar.contains("正月初一"));
}
