use super::*;

fn items(list: &[&str]) -> ExpiryArgs {
    ExpiryArgs {
        items: list.iter().map(|s| s.to_string()).collect(),
        today: Some("2026-10-28".into()),
        soon_days: 3,
    }
}

#[test]
fn expiry_sorts_by_date_and_classifies_like_the_board() {
    let rows = expiry(items(&[
        "Milk:2026-10-30",
        "Eggs:2026-11-20",
        "Ham:2026-10-27",
    ]))
    .unwrap();
    let got: Vec<(&str, &str, i64)> = rows
        .iter()
        .map(|r| (r.name.as_str(), r.state.as_str(), r.days))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Ham", "overdue", -1),
            ("Milk", "soon", 2),
            ("Eggs", "ok", 23)
        ]
    );
}

#[test]
fn names_may_contain_colons() {
    let rows = expiry(items(&["Sauce: chili:2026-10-29"])).unwrap();
    assert_eq!(rows[0].name, "Sauce: chili");
}

#[test]
fn today_defaults_to_the_local_date() {
    let mut a = items(&["Milk:2026-10-30"]);
    a.today = None;
    assert!(expiry(a).is_ok());
}

#[test]
fn bad_expiry_input_is_rejected() {
    assert_eq!(
        expiry(items(&["Milk"])).unwrap_err().code,
        "fridge.bad_item"
    );
    assert_eq!(
        expiry(items(&["Milk:2026-02-30"])).unwrap_err().code,
        "fridge.bad_date"
    );
    assert_eq!(expiry(items(&[])).unwrap_err().code, "fridge.bad_items");
    assert_eq!(
        expiry(items(&[":2026-10-30"])).unwrap_err().code,
        "fridge.name_required"
    );
}

#[test]
fn convert_within_a_dimension_only() {
    let out = convert(ConvertArgs {
        amount: 1.5,
        from: "kg".into(),
        to: "g".into(),
    })
    .unwrap();
    assert_eq!(out["value"], serde_json::json!(1500.0));
    let out = convert(ConvertArgs {
        amount: 250.0,
        from: "ml".into(),
        to: "L".into(),
    })
    .unwrap();
    assert_eq!(out["value"], serde_json::json!(0.25));
    assert_eq!(
        convert(ConvertArgs {
            amount: 1.0,
            from: "kg".into(),
            to: "ml".into()
        })
        .unwrap_err()
        .code,
        "fridge.unit_mismatch"
    );
    assert_eq!(
        convert(ConvertArgs {
            amount: 1.0,
            from: "pack".into(),
            to: "piece".into()
        })
        .unwrap_err()
        .code,
        "fridge.unit_mismatch"
    );
    assert_eq!(
        convert(ConvertArgs {
            amount: 1.0,
            from: "cup".into(),
            to: "ml".into()
        })
        .unwrap_err()
        .code,
        "fridge.bad_unit"
    );
    assert_eq!(
        convert(ConvertArgs {
            amount: -1.0,
            from: "g".into(),
            to: "kg".into()
        })
        .unwrap_err()
        .code,
        "fridge.bad_amount"
    );
}
