use super::*;

fn args(items: &[&str], people: i64, days: i64) -> ChecklistArgs {
    ChecklistArgs {
        items: items.iter().map(|s| s.to_string()).collect(),
        people,
        days,
    }
}

#[test]
fn rules_multiply_by_people_and_days_but_shared_items_do_not() {
    let out = checklist(args(
        &[
            "Tent:1",
            "Toothbrush:1:p",
            "Socks:1:pd:50",
            "Snack bar:2:d:40",
        ],
        3,
        4,
    ))
    .unwrap();
    let got: Vec<(&str, i64)> = out
        .items
        .iter()
        .map(|r| (r.name.as_str(), r.quantity))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Tent", 1),
            ("Toothbrush", 3),
            ("Socks", 12),
            ("Snack bar", 8)
        ]
    );
    // 已知重量：12 × 50 + 8 × 40 = 920 克
    assert_eq!(out.total_weight, grams(920));
    assert_eq!(out.unknown_weight_items, 2);
}

#[test]
fn malformed_lines_and_ranges_are_rejected() {
    assert_eq!(
        checklist(args(&["Tent"], 1, 1)).unwrap_err().code,
        "plc.bad_line"
    );
    assert_eq!(
        checklist(args(&["Tent:x"], 1, 1)).unwrap_err().code,
        "plc.bad_line"
    );
    assert_eq!(
        checklist(args(&["Tent:1:w"], 1, 1)).unwrap_err().code,
        "plc.bad_line"
    );
    assert_eq!(
        checklist(args(&["Tent:1"], 0, 1)).unwrap_err().code,
        "plc.people_range"
    );
    assert_eq!(
        checklist(args(&["Tent:1::-5"], 1, 1)).unwrap_err().code,
        "plc.weight_range"
    );
}
