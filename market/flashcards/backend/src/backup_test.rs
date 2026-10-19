use super::*;

fn card(id: &str, question: &str) -> BackupCard {
    BackupCard {
        id: id.to_owned(),
        deck: "d".to_owned(),
        question: question.to_owned(),
        answer: "a".to_owned(),
        tags: vec![],
        paused: false,
        stage: 1,
        due: "2026-10-20".to_owned(),
        history: vec![],
        created_at: "2026-10-19".to_owned(),
    }
}

#[test]
fn export_and_preview_conflicts() {
    let exported = export(ExportArgs {
        cards: vec![card("1", "Q1"), card("2", "Q2")],
    })
    .unwrap();
    assert_eq!(exported.count, 2);
    // 当前有同 id 但内容不同 → 冲突；全新 → fresh
    let preview = restore_preview(PreviewArgs {
        json: exported.json.clone(),
        current: vec![card("1", "Q1-修改")],
    })
    .unwrap();
    assert_eq!(preview.incoming, 2);
    assert_eq!(preview.fresh, 1);
    assert_eq!(preview.conflicts.len(), 1);
    assert_eq!(preview.conflicts[0].id, "1");
}

#[test]
fn apply_modes() {
    let exported = export(ExportArgs {
        cards: vec![card("1", "Q1-新"), card("2", "Q2")],
    })
    .unwrap();
    let current = vec![card("1", "Q1-旧"), card("3", "Q3")];
    // overwrite：备份覆盖同 id，保留未涉及
    let out = restore_apply(ApplyArgs {
        json: exported.json.clone(),
        current: current.clone(),
        mode: "overwrite".to_owned(),
    })
    .unwrap();
    assert_eq!(out.cards.len(), 3);
    assert_eq!(out.overwritten, 1);
    assert_eq!(out.skipped, 0);
    assert_eq!(
        out.cards.iter().find(|c| c.id == "1").unwrap().question,
        "Q1-新"
    );
    // skip：保留现状
    let out = restore_apply(ApplyArgs {
        json: exported.json.clone(),
        current: current.clone(),
        mode: "skip".to_owned(),
    })
    .unwrap();
    assert_eq!(out.skipped, 1);
    assert_eq!(
        out.cards.iter().find(|c| c.id == "1").unwrap().question,
        "Q1-旧"
    );
    // replace：整体替换
    let out = restore_apply(ApplyArgs {
        json: exported.json.clone(),
        current,
        mode: "replace".to_owned(),
    })
    .unwrap();
    assert_eq!(out.cards.len(), 2);
}

#[test]
fn invalid_backup_rejected() {
    let err = restore_preview(PreviewArgs {
        json: "not json".to_owned(),
        current: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.bad_backup");
    let err = restore_preview(PreviewArgs {
        json: r#"{"version":2,"exportedAt":"","cards":[]}"#.to_owned(),
        current: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.bad_version");
    let err = restore_apply(ApplyArgs {
        json: export(ExportArgs { cards: vec![] }).unwrap().json,
        current: vec![],
        mode: "merge".to_owned(),
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.bad_mode");
}

#[test]
fn progress_fields_survive_roundtrip() {
    let mut learned = card("1", "Q");
    learned.stage = 4;
    learned.due = "2026-11-18".to_owned();
    learned.history = vec![serde_json::json!({"date": "2026-10-19", "remembered": true})];
    let exported = export(ExportArgs {
        cards: vec![learned],
    })
    .unwrap();
    let out = restore_apply(ApplyArgs {
        json: exported.json,
        current: vec![],
        mode: "replace".to_owned(),
    })
    .unwrap();
    let restored = &out.cards[0];
    assert_eq!((restored.stage, restored.due.as_str()), (4, "2026-11-18"));
    assert_eq!(restored.history.len(), 1);
}
