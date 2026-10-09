use super::*;

#[test]
fn accepts_common_date_forms() {
    for input in [
        "2024-02-29",
        "2024/2/29",
        "2024.02.29",
        "20240229",
        " 2024-2-29 ",
    ] {
        let m = moment(input).unwrap();
        assert_eq!(m.at.date(), Date::new(2024, 2, 29).unwrap(), "{input}");
        assert!(!m.has_time);
    }
}

#[test]
fn accepts_times() {
    let m = moment("2024-02-29 08:30").unwrap();
    assert!(m.has_time);
    assert_eq!(format(m.at, true), "2024-02-29 08:30:00");
    assert_eq!(
        format(moment("2024-02-29T23:59:58").unwrap().at, true),
        "2024-02-29 23:59:58"
    );
}

#[test]
fn rejects_invalid_dates() {
    for input in [
        "",
        "2023-02-29",
        "2024-13-01",
        "abc",
        "2024-01-01 25:00",
        "2024-01-01 10",
        "1 2 3",
    ] {
        assert_eq!(moment(input).unwrap_err().code, "date.invalid", "{input}");
    }
}
