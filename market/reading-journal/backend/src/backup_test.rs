use super::*;
use crate::ops::Op;
use crate::ops::tests::{empty, ok, reading};

#[test]
fn restoring_the_same_backup_twice_does_not_double_count_sessions() {
    let (j, _, session) = reading(Some(100));
    let (j, _) = ok(
        &j,
        Op::Complete {
            session_id: session,
            finish: Some("2026-10-10".into()),
        },
        "op-c",
    );
    let text = backup_json(&j, &BTreeMap::new(), "2026-10-28").unwrap();
    let once = restore(&empty(), &text, Mode::Merge, 0, false)
        .unwrap()
        .journal
        .unwrap();
    let twice = restore(&once, &text, Mode::Merge, once.revision, false).unwrap();
    assert_eq!(twice.report.sessions_skipped, 1);
    let twice = twice.journal.unwrap();
    assert_eq!(twice.sessions.len(), 1);
    assert_eq!(crate::view::review(&twice, 2026).unwrap().completions, 1);
}

#[test]
fn replace_keeps_work_and_session_links() {
    let (j, work, _) = reading(None);
    let text = backup_json(&j, &BTreeMap::new(), "2026-10-28").unwrap();
    let out = restore(&empty(), &text, Mode::Replace, 0, false)
        .unwrap()
        .journal
        .unwrap();
    assert_eq!(out.sessions_of(&work).count(), 1);
}

#[test]
fn broken_backups_are_rejected() {
    assert_eq!(
        restore(&empty(), "{}", Mode::Merge, 0, true)
            .unwrap_err()
            .code,
        "journal.bad_backup"
    );
    let big = " ".repeat(MAX_BACKUP + 1);
    assert_eq!(
        restore(&empty(), &big, Mode::Merge, 0, true)
            .unwrap_err()
            .code,
        "journal.backup_too_large"
    );
    let (mut j, _, _) = reading(Some(10));
    j.sessions[0].progress = 50;
    let text = serde_json::to_string(&Backup {
        format: FORMAT.into(),
        version: SCHEMA,
        exported_at: "x".into(),
        works: j.works,
        sessions: j.sessions,
        covers: BTreeMap::new(),
    })
    .unwrap();
    assert_eq!(
        restore(&empty(), &text, Mode::Merge, 0, true)
            .unwrap_err()
            .code,
        "journal.over_total"
    );
}

#[test]
fn csv_has_one_row_per_session_and_guards_formulas() {
    let (mut j, _, _) = reading(None);
    j.sessions[0].review = "=cmd".into();
    let headers: Vec<String> = (0..14).map(|i| format!("h{i}")).collect();
    let csv = list_csv(
        &j,
        &headers,
        &BTreeMap::from([("book".to_string(), "Book".to_string())]),
    )
    .unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert_eq!(csv.lines().count(), 2);
    assert!(csv.contains("\"Book\""));
    assert!(csv.contains("\"'=cmd\""));
}
