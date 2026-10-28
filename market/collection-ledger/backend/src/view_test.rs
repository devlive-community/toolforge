use super::*;
use crate::ops::tests::with_item;
use crate::ops::{Op, Request, apply};

fn lend(l: &Ledger, item: &str, qty: u32, due: Option<&str>, op: &str) -> Ledger {
    apply(Request {
        ledger: l.clone(),
        op: Op::Lend {
            item_id: item.into(),
            borrower: "Kim".into(),
            qty,
            date: "2026-10-20".into(),
            due: due.map(String::from),
            note: String::new(),
        },
        op_id: op.into(),
        revision: l.revision,
        today: "2026-10-28".into(),
        preview: false,
    })
    .unwrap()
    .ledger
    .unwrap()
}

fn query(l: &Ledger, state: State) -> Query {
    Query {
        ledger: l.clone(),
        today: "2026-10-28".into(),
        search: String::new(),
        location: None,
        category: None,
        state,
        offset: 0,
        limit: 20,
        pending_offset: 0,
        pending_limit: 20,
    }
}

#[test]
fn pending_lists_overdue_first_then_soon_then_undated() {
    let (l, item) = with_item(10);
    let l = lend(&l, &item, 1, None, "a");
    let l = lend(&l, &item, 1, Some("2026-10-30"), "b");
    let l = lend(&l, &item, 1, Some("2026-10-25"), "c");
    let v = view(&query(&l, State::All)).unwrap();
    let states: Vec<DueState> = v.pending.iter().map(|p| p.due_state).collect();
    assert_eq!(
        states,
        vec![DueState::Overdue, DueState::Soon, DueState::None]
    );
    assert_eq!(v.summary.overdue, 1);
    assert_eq!(v.summary.outstanding, 3);
    assert_eq!(v.summary.available, 7);
}

#[test]
fn state_filters_split_in_hand_lent_and_archived() {
    let (l, item) = with_item(1);
    let l = lend(&l, &item, 1, None, "a");
    assert_eq!(view(&query(&l, State::InHand)).unwrap().total, 0);
    assert_eq!(view(&query(&l, State::Lent)).unwrap().total, 1);
    assert_eq!(view(&query(&l, State::Archived)).unwrap().total, 0);
}

#[test]
fn search_location_and_paging() {
    let (l, _) = with_item(1);
    let mut q = query(&l, State::All);
    q.search = "catan".into();
    assert_eq!(view(&q).unwrap().total, 1);
    q.search = "chess".into();
    assert_eq!(view(&q).unwrap().total, 0);
    q.search.clear();
    q.location = Some("Shelf A".into());
    let v = view(&q).unwrap();
    assert_eq!(v.total, 1);
    assert_eq!(v.locations, vec!["Shelf A"]);
    q.offset = 5;
    assert!(view(&q).unwrap().items.is_empty());
}

#[test]
fn history_lists_every_loan_of_the_item() {
    let (l, item) = with_item(5);
    let l = lend(&l, &item, 2, None, "a");
    let l = lend(&l, &item, 1, None, "b");
    let (v, loans) = history(&l, &item, "2026-10-28").unwrap();
    assert_eq!(v.outstanding, 3);
    assert_eq!(loans.len(), 2);
}
