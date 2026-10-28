use super::*;
use crate::ops::tests::{book, ok, reading};
use crate::ops::{Op, Request, apply};

fn complete(j: &Journal, session: &str, finish: Option<&str>, rating: u8, op: &str) -> Journal {
    let j = apply(Request {
        journal: j.clone(),
        op: Op::Update {
            session_id: session.into(),
            start: Some("2025-12-20".into()),
            finish: None,
            progress: 0,
            rating: Some(rating),
            review: "great".into(),
        },
        op_id: format!("{op}-u"),
        revision: j.revision,
        today: "2026-10-28".into(),
    })
    .unwrap()
    .journal;
    ok(
        &j,
        Op::Complete {
            session_id: session.into(),
            finish: finish.map(String::from),
        },
        op,
    )
    .0
}

#[test]
fn year_review_counts_works_and_completions_separately_by_finish_date() {
    let (j, work, first) = reading(Some(300));
    let j = complete(&j, &first, Some("2026-01-05"), 8, "c1");
    let (j, second) = ok(
        &j,
        Op::Start {
            work_id: work.clone(),
            start: None,
        },
        "s2",
    );
    let j = complete(&j, &second.unwrap(), Some("2026-03-01"), 10, "c2");
    // 跨年：开始于 2025，完成于 2026 才算 2026
    let r = review(&j, 2026).unwrap();
    assert_eq!((r.works, r.completions, r.books), (1, 2, 1));
    assert_eq!(r.pages, 600);
    assert_eq!(r.average_rating, Some(9.0));
    assert_eq!(r.months[0], 1);
    assert_eq!(r.months[2], 1);
    assert_eq!(review(&j, 2025).unwrap().completions, 0);
}

#[test]
fn completions_without_a_finish_date_are_listed_as_pending_not_counted() {
    let (j, _, session) = reading(None);
    let j = complete(&j, &session, None, 6, "c1");
    let r = review(&j, 2026).unwrap();
    assert_eq!(r.completions, 0);
    assert_eq!(r.pending, 1);
    let v = view(&Query {
        journal: j,
        search: String::new(),
        kind: None,
        status: None,
        tag: None,
        year: None,
        min_rating: None,
        offset: 0,
        limit: 20,
    })
    .unwrap();
    assert_eq!(v.pending.len(), 1);
    assert!(v.review_years.is_empty());
}

#[test]
fn filters_search_reviews_tags_and_ratings() {
    let (j, _, session) = reading(None);
    let j = complete(&j, &session, Some("2026-02-02"), 9, "c1");
    let (j, _) = ok(
        &j,
        Op::CreateWork {
            fields: book("Other", None),
            status: Status::Want,
        },
        "w2",
    );
    let q = |search: &str, min: Option<u8>, status: Option<Status>| Query {
        journal: j.clone(),
        search: search.into(),
        kind: None,
        status,
        tag: Some("sci-fi".into()),
        year: Some(2001),
        min_rating: min,
        offset: 0,
        limit: 20,
    };
    assert_eq!(
        view(&q("great", None, None)).unwrap().total,
        1,
        "短评可搜索"
    );
    assert_eq!(view(&q("", Some(9), None)).unwrap().total, 1);
    assert_eq!(view(&q("", Some(10), None)).unwrap().total, 0);
    assert_eq!(view(&q("", None, Some(Status::Want))).unwrap().total, 1);
    let v = view(&q("", None, None)).unwrap();
    assert_eq!(v.counts.done, 1);
    assert_eq!(v.review_years, vec![2026]);
}

#[test]
fn stars_render_half_steps() {
    assert_eq!(stars(Some(7)), "★★★½");
    assert_eq!(stars(Some(10)), "★★★★★");
    assert_eq!(stars(None), "");
}
