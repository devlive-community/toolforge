use super::*;

pub(crate) const TODAY: &str = "2026-10-28";

fn empty() -> Ledger {
    Ledger {
        schema: SCHEMA,
        ..Ledger::default()
    }
}

fn run(ledger: &Ledger, op: Op, op_id: &str) -> PluginResult<Outcome> {
    apply(Request {
        ledger: ledger.clone(),
        op,
        op_id: op_id.into(),
        revision: ledger.revision,
        today: TODAY.into(),
        preview: false,
    })
}

fn ok(ledger: &Ledger, op: Op, op_id: &str) -> Ledger {
    run(ledger, op, op_id).unwrap().ledger.unwrap()
}

fn fields(name: &str) -> Fields {
    Fields {
        name: name.into(),
        location: "Shelf A".into(),
        category: "Board games".into(),
        ..Fields::default()
    }
}

/// 新建一件 owned 份的藏品，返回台账与藏品编号
pub(crate) fn with_item(owned: u32) -> (Ledger, String) {
    let out = run(
        &empty(),
        Op::Create {
            fields: fields("Catan"),
            owned,
        },
        "op-create",
    )
    .unwrap();
    (out.ledger.unwrap(), out.created_id.unwrap())
}

fn lend(item: &str, qty: u32) -> Op {
    Op::Lend {
        item_id: item.into(),
        borrower: "Alex".into(),
        qty,
        date: "2026-10-20".into(),
        due: Some("2026-10-27".into()),
        note: String::new(),
    }
}

fn give_back(loan: &str, qty: u32) -> Op {
    Op::Return {
        loan_id: loan.into(),
        qty,
        date: "2026-10-25".into(),
        note: String::new(),
    }
}

/// 守恒：在手 + 未还 = 保有
fn conserved(ledger: &Ledger) {
    for item in &ledger.items {
        assert_eq!(
            ledger.available(item) + ledger.outstanding(&item.id),
            item.owned,
            "{} 不守恒",
            item.name
        );
    }
}

#[test]
fn several_loans_and_partial_returns_keep_counts_conserved() {
    let (l, item) = with_item(5);
    let out = run(&l, lend(&item, 2), "op-1").unwrap();
    let loan1 = out.created_id.unwrap();
    let l = out.ledger.unwrap();
    let out = run(&l, lend(&item, 3), "op-2").unwrap();
    let loan2 = out.created_id.unwrap();
    let l = out.ledger.unwrap();
    assert_eq!(l.available(l.item(&item).unwrap()), 0);
    let l = ok(&l, give_back(&loan1, 1), "op-3");
    let l = ok(&l, give_back(&loan2, 3), "op-4");
    conserved(&l);
    assert_eq!(l.outstanding(&item), 1);
    assert_eq!(l.loan(&loan1).unwrap().outstanding(), 1);
    assert_eq!(l.loan(&loan2).unwrap().outstanding(), 0);
}

#[test]
fn cannot_lend_more_than_in_hand_or_return_more_than_owed() {
    let (l, item) = with_item(2);
    assert_eq!(
        run(&l, lend(&item, 3), "x").unwrap_err().code,
        "ledger.over_available"
    );
    assert_eq!(
        run(&l, lend(&item, 0), "x").unwrap_err().code,
        "ledger.over_available"
    );
    let out = run(&l, lend(&item, 2), "op-1").unwrap();
    let loan = out.created_id.unwrap();
    let l = out.ledger.unwrap();
    assert_eq!(
        run(&l, give_back(&loan, 3), "x").unwrap_err().code,
        "ledger.over_return"
    );
    let l = ok(&l, give_back(&loan, 2), "op-2");
    assert_eq!(
        run(&l, give_back(&loan, 1), "x").unwrap_err().code,
        "ledger.over_return"
    );
}

#[test]
fn duplicate_submission_applies_once() {
    let (l, item) = with_item(3);
    let first = run(&l, lend(&item, 1), "op-same").unwrap().ledger.unwrap();
    // 界面重试：同一操作编号再次提交（基于已更新的台账）
    let again = run(&first, lend(&item, 1), "op-same").unwrap();
    assert!(again.duplicate);
    assert_eq!(again.ledger.unwrap().loans.len(), 1);
}

#[test]
fn stale_revision_is_rejected() {
    let (l, item) = with_item(3);
    let mut req = Request {
        ledger: l.clone(),
        op: lend(&item, 1),
        op_id: "op-1".into(),
        revision: l.revision + 5,
        today: TODAY.into(),
        preview: false,
    };
    assert_eq!(apply(req.clone()).unwrap_err().code, "ledger.stale");
    req.revision = l.revision;
    assert!(apply(req).is_ok());
}

#[test]
fn preview_reports_impact_without_changing_anything() {
    let (l, item) = with_item(4);
    let out = apply(Request {
        ledger: l.clone(),
        op: lend(&item, 3),
        op_id: "op-1".into(),
        revision: l.revision,
        today: TODAY.into(),
        preview: true,
    })
    .unwrap();
    assert!(out.ledger.is_none());
    assert_eq!(out.impact.available, [4, 1]);
    assert_eq!(out.impact.outstanding, [0, 3]);
    assert_eq!(out.impact.loan_outstanding, Some([0, 3]));
    assert_eq!(out.impact.borrower.as_deref(), Some("Alex"));
}

#[test]
fn undo_reverts_the_last_loan_and_the_last_return() {
    let (l, item) = with_item(3);
    let out = run(&l, lend(&item, 2), "op-1").unwrap();
    let loan = out.created_id.unwrap();
    let l = out.ledger.unwrap();
    let l = ok(&l, give_back(&loan, 1), "op-2");
    assert_eq!(l.undo.as_ref().unwrap().kind, "return");
    let l = ok(&l, Op::Undo, "op-3");
    assert_eq!(l.loan(&loan).unwrap().returns.len(), 0);
    conserved(&l);
    // 撤销只有一步，不能再撤销
    assert_eq!(
        run(&l, Op::Undo, "op-4").unwrap_err().code,
        "ledger.nothing_to_undo"
    );
}

#[test]
fn undo_of_a_new_loan_removes_it_and_restores_availability() {
    let (l, item) = with_item(3);
    let l = ok(&l, lend(&item, 3), "op-1");
    let l = ok(&l, Op::Undo, "op-2");
    assert!(l.loans.is_empty());
    assert_eq!(l.available(l.item(&item).unwrap()), 3);
}

#[test]
fn owned_cannot_drop_below_outstanding() {
    let (l, item) = with_item(5);
    let l = ok(&l, lend(&item, 3), "op-1");
    let err = run(
        &l,
        Op::SetOwned {
            item_id: item.clone(),
            owned: 2,
        },
        "x",
    )
    .unwrap_err();
    assert_eq!(err.code, "ledger.below_outstanding");
    let l = ok(
        &l,
        Op::SetOwned {
            item_id: item.clone(),
            owned: 3,
        },
        "op-2",
    );
    assert_eq!(l.available(l.item(&item).unwrap()), 0);
    conserved(&l);
}

#[test]
fn items_with_loans_are_archived_not_deleted() {
    let (l, item) = with_item(2);
    let out = run(&l, lend(&item, 1), "op-1").unwrap();
    let loan = out.created_id.unwrap();
    let l = out.ledger.unwrap();
    assert_eq!(
        run(
            &l,
            Op::Delete {
                item_id: item.clone()
            },
            "x"
        )
        .unwrap_err()
        .code,
        "ledger.delete_with_loans"
    );
    // 借出未还时也不能归档
    assert_eq!(
        run(
            &l,
            Op::Archive {
                item_id: item.clone(),
                archived: true
            },
            "x"
        )
        .unwrap_err()
        .code,
        "ledger.archive_lent"
    );
    let l = ok(&l, give_back(&loan, 1), "op-2");
    let l = ok(
        &l,
        Op::Archive {
            item_id: item.clone(),
            archived: true,
        },
        "op-3",
    );
    assert!(l.item(&item).unwrap().archived);
    assert_eq!(l.loans.len(), 1, "归档保留往来历史");
    // 归档的藏品不能再借出
    assert_eq!(
        run(&l, lend(&item, 1), "x").unwrap_err().code,
        "ledger.archived"
    );
}

#[test]
fn items_without_loans_can_be_deleted() {
    let (l, item) = with_item(1);
    let l = ok(&l, Op::Delete { item_id: item }, "op-1");
    assert!(l.items.is_empty());
    assert!(l.undo.is_none(), "删除不可撤销");
}

#[test]
fn dates_are_validated() {
    let (l, item) = with_item(2);
    let mut future = lend(&item, 1);
    if let Op::Lend { date, .. } = &mut future {
        *date = "2026-11-01".into();
    }
    assert_eq!(run(&l, future, "x").unwrap_err().code, "ledger.future_date");
    let mut early_due = lend(&item, 1);
    if let Op::Lend { due, .. } = &mut early_due {
        *due = Some("2026-10-01".into());
    }
    assert_eq!(
        run(&l, early_due, "x").unwrap_err().code,
        "ledger.due_before_loan"
    );
    let out = run(&l, lend(&item, 1), "op-1").unwrap();
    let loan = out.created_id.unwrap();
    let l = out.ledger.unwrap();
    let early = Op::Return {
        loan_id: loan,
        qty: 1,
        date: "2026-10-01".into(),
        note: String::new(),
    };
    assert_eq!(
        run(&l, early, "x").unwrap_err().code,
        "ledger.return_before_loan"
    );
}

#[test]
fn same_name_different_editions_are_separate_items() {
    let (l, _) = with_item(1);
    let mut second = fields("Catan");
    second.edition = "5th".into();
    let l = ok(
        &l,
        Op::Create {
            fields: second,
            owned: 2,
        },
        "op-2",
    );
    assert_eq!(l.items.len(), 2);
    assert_ne!(l.items[0].id, l.items[1].id);
}

#[test]
fn edits_keep_quantities_and_flags() {
    let (l, item) = with_item(4);
    let l = ok(&l, lend(&item, 1), "op-1");
    let mut f = fields("Catan (big box)");
    f.location = "Shelf B".into();
    let l = ok(
        &l,
        Op::Edit {
            item_id: item.clone(),
            fields: f,
        },
        "op-2",
    );
    let edited = l.item(&item).unwrap();
    assert_eq!(edited.owned, 4);
    assert_eq!(edited.location, "Shelf B");
    conserved(&l);
}
