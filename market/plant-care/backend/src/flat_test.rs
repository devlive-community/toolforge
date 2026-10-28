use super::*;

fn args(items: &[&str]) -> DueArgs {
    DueArgs {
        items: items.iter().map(|s| s.to_string()).collect(),
        today: Some("2026-10-28".into()),
    }
}

#[test]
fn next_date_is_last_actual_date_plus_interval() {
    let rows = due(
        args(&["Fern:7:2026-10-25", "Cactus:30:2026-09-01", "Basil:2"]),
        "1970-01-01",
    )
    .unwrap();
    let got: Vec<(&str, &str, i64, &str)> = rows
        .iter()
        .map(|r| (r.name.as_str(), r.next.as_str(), r.days, r.state.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Cactus", "2026-10-01", -27, "overdue"),
            ("Basil", "2026-10-28", 0, "today"),
            ("Fern", "2026-11-01", 4, "upcoming"),
        ]
    );
}

#[test]
fn today_falls_back_to_the_local_date() {
    let mut a = args(&["Fern:7:2026-10-25"]);
    a.today = None;
    let rows = due(a, "2026-10-30").unwrap();
    assert_eq!(rows[0].days, 2);
}

#[test]
fn invalid_items_are_rejected_with_app_rules() {
    assert_eq!(
        due(args(&["Fern"]), "").unwrap_err().code,
        "plant-care.format"
    );
    assert_eq!(
        due(args(&["Fern:0"]), "").unwrap_err().code,
        "plant-care.interval"
    );
    assert_eq!(
        due(args(&["Fern:9999"]), "").unwrap_err().code,
        "plant-care.interval"
    );
    assert_eq!(
        due(args(&["Fern:7:2026-02-30"]), "").unwrap_err().code,
        "plant-care.date"
    );
    assert_eq!(
        due(args(&["Fern:7:2026-11-30"]), "").unwrap_err().code,
        "plant-care.future"
    );
    assert_eq!(due(args(&[":7"]), "").unwrap_err().code, "plant-care.text");
}
