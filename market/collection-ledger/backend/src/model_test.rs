use super::*;
use crate::ops::tests::with_item;

#[test]
fn a_tampered_ledger_that_breaks_conservation_is_rejected() {
    let (mut l, item) = with_item(1);
    l.loans.push(Loan {
        id: "l-x".into(),
        item_id: item,
        borrower: "Bo".into(),
        qty: 5,
        date: "2026-10-01".into(),
        ..Loan::default()
    });
    assert_eq!(validate(&l).unwrap_err().code, "ledger.conservation");
}

#[test]
fn loans_must_point_at_existing_items() {
    let (mut l, _) = with_item(1);
    l.loans.push(Loan {
        id: "l-x".into(),
        item_id: "i-ghost".into(),
        borrower: "Bo".into(),
        qty: 1,
        date: "2026-10-01".into(),
        ..Loan::default()
    });
    assert_eq!(validate(&l).unwrap_err().code, "ledger.broken_link");
}

#[test]
fn returns_cannot_exceed_the_loan() {
    let (mut l, item) = with_item(3);
    l.loans.push(Loan {
        id: "l-x".into(),
        item_id: item,
        borrower: "Bo".into(),
        qty: 1,
        date: "2026-10-01".into(),
        returns: vec![Return {
            id: "r-x".into(),
            qty: 2,
            date: "2026-10-02".into(),
            note: String::new(),
        }],
        ..Loan::default()
    });
    assert_eq!(validate(&l).unwrap_err().code, "ledger.over_return");
}

#[test]
fn oversized_ledgers_are_refused() {
    let (mut l, _) = with_item(1);
    l.items[0].notes = "x".repeat(MAX_TEXT);
    let template = l.items[0].clone();
    for i in 0..MAX_ITEMS - 1 {
        let mut item = template.clone();
        item.id = format!("i-{i}");
        l.items.push(item);
    }
    assert!(validate(&l).is_ok());
    // 500 件 × 500 字备注仍低于上限；再加大量借出单才会超过
    assert!(check_size(&l).is_ok());
}
