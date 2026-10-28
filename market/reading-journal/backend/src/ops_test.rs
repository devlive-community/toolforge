use super::*;

pub(crate) const TODAY: &str = "2026-10-28";

pub(crate) fn empty() -> Journal {
    Journal {
        schema: SCHEMA,
        ..Journal::default()
    }
}

pub(crate) fn run(j: &Journal, op: Op, op_id: &str) -> PluginResult<Outcome> {
    apply(Request {
        journal: j.clone(),
        op,
        op_id: op_id.into(),
        revision: j.revision,
        today: TODAY.into(),
    })
}

pub(crate) fn ok(j: &Journal, op: Op, op_id: &str) -> (Journal, Option<String>) {
    let out = run(j, op, op_id).unwrap();
    (out.journal, out.created_id)
}

pub(crate) fn book(title: &str, total: Option<u32>) -> Fields {
    Fields {
        kind: Kind::Book,
        title: title.into(),
        creator: "Author".into(),
        year: Some(2001),
        tags: vec!["sci-fi".into()],
        total,
        ..Fields::default()
    }
}

/// 新建一本书并开始一次阅读，返回手账、作品编号、经历编号
pub(crate) fn reading(total: Option<u32>) -> (Journal, String, String) {
    let (j, work) = ok(
        &empty(),
        Op::CreateWork {
            fields: book("Dune", total),
            status: Status::Want,
        },
        "op-w",
    );
    let work = work.unwrap();
    let (j, session) = ok(
        &j,
        Op::Start {
            work_id: work.clone(),
            start: Some("2026-10-01".into()),
        },
        "op-s",
    );
    (j, work, session.unwrap())
}

fn update(session: &str, progress: u32) -> Op {
    Op::Update {
        session_id: session.into(),
        start: Some("2026-10-01".into()),
        finish: None,
        progress,
        rating: Some(8),
        review: "Spice!".into(),
    }
}

#[test]
fn reaching_the_last_page_does_not_complete_the_session() {
    let (j, work, session) = reading(Some(400));
    let (j, _) = ok(&j, update(&session, 400), "op-1");
    let s = j.session(&session).unwrap();
    assert!(!s.completed, "进度到最后一页不偷偷算完成");
    assert_eq!(j.work(&work).unwrap().status, Status::Doing);
}

#[test]
fn progress_cannot_exceed_a_known_total() {
    let (j, _, session) = reading(Some(400));
    assert_eq!(
        run(&j, update(&session, 401), "x").unwrap_err().code,
        "journal.over_total"
    );
    // 总量未知时不限制（不伪造百分比）
    let (j, _, session) = reading(None);
    assert!(run(&j, update(&session, 9999), "x").is_ok());
}

#[test]
fn completion_is_explicit_and_can_be_undone() {
    let (j, work, session) = reading(Some(400));
    let (j, _) = ok(
        &j,
        Op::Complete {
            session_id: session.clone(),
            finish: Some("2026-10-20".into()),
        },
        "op-1",
    );
    let s = j.session(&session).unwrap();
    assert!(s.completed);
    assert_eq!(s.progress, 400);
    assert_eq!(j.work(&work).unwrap().status, Status::Done);
    assert_eq!(
        run(
            &j,
            Op::Complete {
                session_id: session.clone(),
                finish: None
            },
            "x"
        )
        .unwrap_err()
        .code,
        "journal.already_completed"
    );
    let (j, _) = ok(
        &j,
        Op::Uncomplete {
            session_id: session.clone(),
        },
        "op-2",
    );
    assert!(!j.session(&session).unwrap().completed);
    assert_eq!(j.session(&session).unwrap().finish, None);
    assert_eq!(j.work(&work).unwrap().status, Status::Doing);
}

#[test]
fn finish_cannot_precede_start_or_be_in_the_future() {
    let (j, _, session) = reading(Some(400));
    let early = Op::Complete {
        session_id: session.clone(),
        finish: Some("2026-09-01".into()),
    };
    assert_eq!(
        run(&j, early, "x").unwrap_err().code,
        "journal.finish_before_start"
    );
    let future = Op::Complete {
        session_id: session.clone(),
        finish: Some("2026-12-01".into()),
    };
    assert_eq!(
        run(&j, future, "x").unwrap_err().code,
        "journal.future_date"
    );
    // 未完成的经历不能单独填完成日
    let mut with_finish = update(&session, 10);
    if let Op::Update { finish, .. } = &mut with_finish {
        *finish = Some("2026-10-10".into());
    }
    assert_eq!(
        run(&j, with_finish, "x").unwrap_err().code,
        "journal.finish_without_complete"
    );
}

#[test]
fn rereading_adds_a_new_session_and_keeps_the_old_review() {
    let (j, work, first) = reading(Some(400));
    let (j, _) = ok(&j, update(&first, 120), "op-1");
    let (j, _) = ok(
        &j,
        Op::Complete {
            session_id: first.clone(),
            finish: Some("2026-10-20".into()),
        },
        "op-2",
    );
    let (j, second) = ok(
        &j,
        Op::Start {
            work_id: work.clone(),
            start: None,
        },
        "op-3",
    );
    let second = second.unwrap();
    assert_ne!(first, second);
    assert_eq!(j.sessions_of(&work).count(), 2);
    assert_eq!(j.session(&first).unwrap().review, "Spice!");
    assert!(j.session(&first).unwrap().completed);
}

#[test]
fn editing_the_work_keeps_reviews_and_total_still_limits_progress() {
    let (j, work, session) = reading(Some(400));
    let (j, _) = ok(&j, update(&session, 300), "op-1");
    let (j, _) = ok(
        &j,
        Op::EditWork {
            work_id: work.clone(),
            fields: book("Dune (Deluxe)", Some(500)),
        },
        "op-2",
    );
    assert_eq!(j.session(&session).unwrap().review, "Spice!");
    // 总页数改小到低于已读页数：拒绝
    let err = run(
        &j,
        Op::EditWork {
            work_id: work,
            fields: book("Dune", Some(200)),
        },
        "x",
    )
    .unwrap_err();
    assert_eq!(err.code, "journal.over_total");
}

#[test]
fn same_titles_stay_separate() {
    let (j, a) = ok(
        &empty(),
        Op::CreateWork {
            fields: book("Solaris", None),
            status: Status::Want,
        },
        "op-a",
    );
    let mut film = book("Solaris", Some(167));
    film.kind = Kind::Film;
    let (j, b) = ok(
        &j,
        Op::CreateWork {
            fields: film,
            status: Status::Want,
        },
        "op-b",
    );
    assert_ne!(a, b);
    assert_eq!(j.works.len(), 2);
}

#[test]
fn duplicate_and_stale_submissions() {
    let (j, _, session) = reading(Some(400));
    let (j2, _) = ok(&j, update(&session, 10), "op-same");
    let again = run(&j2, update(&session, 20), "op-same").unwrap();
    assert!(again.duplicate);
    assert_eq!(again.journal.session(&session).unwrap().progress, 10);
    let stale = apply(Request {
        journal: j2.clone(),
        op: update(&session, 5),
        op_id: "x".into(),
        revision: 0,
        today: TODAY.into(),
    });
    assert_eq!(stale.unwrap_err().code, "journal.stale");
}

#[test]
fn undo_restores_both_session_and_work_status() {
    let (j, work, session) = reading(Some(400));
    let (j, _) = ok(
        &j,
        Op::Complete {
            session_id: session.clone(),
            finish: Some("2026-10-20".into()),
        },
        "op-1",
    );
    let (j, _) = ok(&j, Op::Undo, "op-2");
    assert!(!j.session(&session).unwrap().completed);
    assert_eq!(j.work(&work).unwrap().status, Status::Doing);
    assert_eq!(
        run(&j, Op::Undo, "x").unwrap_err().code,
        "journal.nothing_to_undo"
    );
}

#[test]
fn works_with_sessions_cannot_be_deleted() {
    let (j, work, session) = reading(None);
    assert_eq!(
        run(
            &j,
            Op::DeleteWork {
                work_id: work.clone()
            },
            "x"
        )
        .unwrap_err()
        .code,
        "journal.delete_with_sessions"
    );
    let (j, _) = ok(
        &j,
        Op::DeleteSession {
            session_id: session,
        },
        "op-1",
    );
    let (j, _) = ok(&j, Op::DeleteWork { work_id: work }, "op-2");
    assert!(j.works.is_empty());
}

#[test]
fn bad_ratings_tags_and_reviews_are_rejected() {
    let (j, _, session) = reading(None);
    let mut bad = update(&session, 1);
    if let Op::Update { rating, .. } = &mut bad {
        *rating = Some(11);
    }
    assert_eq!(run(&j, bad, "x").unwrap_err().code, "journal.bad_rating");
    let mut long = update(&session, 1);
    if let Op::Update { review, .. } = &mut long {
        *review = "x".repeat(MAX_REVIEW + 1);
    }
    assert_eq!(
        run(&j, long, "x").unwrap_err().code,
        "journal.review_too_long"
    );
    let mut tags = book("T", None);
    tags.tags = vec!["a".into(), "a".into()];
    assert_eq!(
        run(
            &j,
            Op::CreateWork {
                fields: tags,
                status: Status::Want
            },
            "x"
        )
        .unwrap_err()
        .code,
        "journal.bad_tags"
    );
}
