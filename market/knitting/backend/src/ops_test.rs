use super::*;
use crate::tests::{Context, change, created};
use crate::view::{self, Pages};
use serde_json::{Value, json};

fn part(state: &Workshop) -> &Part {
    state.active().unwrap().1
}
fn inspect(state: &Workshop) -> view::PartView {
    view::inspect(state, Pages::default(), false, &Context::default())
        .unwrap()
        .part
        .unwrap()
}
fn next(state: Workshop) -> Workshop {
    let line = part(&state).completed + 1;
    change(state, json!({"type":"complete","expectedLine":line})).unwrap()
}
fn plan(state: Workshop) -> Workshop {
    change(state,json!({"type":"addReminder","name":"Shape","start":"1","interval":"2","repeats":"2","delta":"2"})).unwrap()
}
fn choose(state: Workshop, id: &str, choice: &str) -> Workshop {
    change(state, json!({"type":"choose","id":id,"choice":choice})).unwrap()
}

#[test]
fn begins_with_zero_completed_and_next_line_one() {
    let state = created();
    let part = inspect(&state);
    assert_eq!(part.completed, 0);
    assert_eq!(part.next_line, 1);
    assert_eq!(part.stitches, 40);
    assert!(part.can_complete);
    assert!(!part.can_undo);
    assert_eq!(part.kind, Kind::Rows);
}

#[test]
fn pattern_last_row_rolls_into_the_next_repeat_and_segment() {
    let mut state = change(
        created(),
        json!({"type":"addSegment","name":"Rib","rows":"4","repeats":"2"}),
    )
    .unwrap();
    state = change(
        state,
        json!({"type":"addSegment","name":"Body","rows":"2","repeats":"3"}),
    )
    .unwrap();
    for _ in 0..3 {
        state = next(state);
    }
    let p = inspect(&state).pattern.unwrap();
    assert_eq!((p.row, p.repeat, p.completed_repeats), (4, 1, 0));
    state = next(state);
    let p = inspect(&state).pattern.unwrap();
    assert_eq!((p.row, p.repeat, p.completed_repeats), (1, 2, 1));
    for _ in 0..4 {
        state = next(state);
    }
    let p = inspect(&state).pattern.unwrap();
    assert_eq!((p.name.as_str(), p.row, p.repeat), ("Body", 1, 1));
    for _ in 0..6 {
        state = next(state);
    }
    let v = inspect(&state);
    assert!(v.pattern_finished);
    assert!(v.pattern.is_none());
    assert_eq!(v.next_line, 15);
    assert_eq!(v.segments.items[0].completed_repeats, 2);
    assert_eq!(v.segments.items[1].completed_repeats, 3);
}

#[test]
fn plans_do_not_change_actual_stitches_and_every_due_choice_is_explicit() {
    let mut state = plan(created());
    let id = part(&state).reminders[0].id.clone();
    let v = inspect(&state);
    assert_eq!(v.due_count, 1);
    assert!(!v.can_complete);
    assert_eq!(v.stitches, 40);
    assert_eq!(
        change(state.clone(), json!({"type":"complete","expectedLine":1}))
            .unwrap_err()
            .code,
        "knitting.pending"
    );
    state = choose(state, &id, "execute");
    let v = inspect(&state);
    assert_eq!(v.stitches, 40);
    assert_eq!(v.projected_stitches, Some(42));
    state = next(state);
    assert_eq!((part(&state).completed, part(&state).stitches), (1, 42));
    assert_eq!(inspect(&state).due_count, 0);
    assert_eq!(inspect(&state).reminders.items[0].executed, 1);
    state = next(state);
    assert_eq!(inspect(&state).due.items[0].occurrence, 2);
    state = choose(state, &id, "skip");
    state = next(state);
    let v = inspect(&state);
    assert_eq!(v.stitches, 42);
    assert_eq!(v.due_count, 0);
    assert_eq!(v.reminders.items[0].next, None);
    assert_eq!(
        (v.reminders.items[0].executed, v.reminders.items[0].skipped),
        (1, 1)
    );
}

#[test]
fn undo_restores_line_cycle_stitches_and_pending_choices_together() {
    let mut state = plan(created());
    let id = part(&state).reminders[0].id.clone();
    state = change(
        state,
        json!({"type":"addSegment","name":"Repeat","rows":"2","repeats":"3"}),
    )
    .unwrap();
    state = choose(state, &id, "execute");
    state = next(state);
    state = next(state);
    state = choose(state, &id, "execute");
    let before = part(&state).snapshot();
    let before_pattern = inspect(&state).pattern.unwrap();
    state = next(state);
    assert_eq!(part(&state).stitches, 44);
    state = change(state, json!({"type":"undo"})).unwrap();
    assert_eq!(part(&state).snapshot(), before);
    let p = inspect(&state).pattern.unwrap();
    assert_eq!(
        (p.row, p.repeat),
        (before_pattern.row, before_pattern.repeat)
    );
    assert_eq!(inspect(&state).reminders.items[0].executed, 1);
    state = change(state, json!({"type":"undo"})).unwrap();
    state = change(state, json!({"type":"undo"})).unwrap();
    assert_eq!((part(&state).completed, part(&state).stitches), (0, 40));
    assert!(!inspect(&state).can_undo);
    assert_eq!(inspect(&state).reminders.items[0].executed, 0);
}

#[test]
fn rejects_negative_stitches_transactionally_and_applies_rule_order() {
    let mut state = change(
        Workshop::default(),
        json!({"type":"newProject","name":"Test","partName":"Body","kind":"rows","stitches":"3"}),
    )
    .unwrap();
    for delta in ["-4", "4"] {
        state=change(state,json!({"type":"addReminder","name":"Shape","start":"1","interval":"1","repeats":"1","delta":delta})).unwrap();
    }
    let ids = part(&state)
        .reminders
        .iter()
        .map(|r| r.id.clone())
        .collect::<Vec<_>>();
    for id in &ids {
        state = choose(state, id, "execute");
    }
    let original = state.clone();
    assert!(!inspect(&state).can_complete);
    assert_eq!(
        change(state.clone(), json!({"type":"complete","expectedLine":1}))
            .unwrap_err()
            .code,
        "knitting.stitches"
    );
    assert_eq!(state, original);
    state = choose(state, &ids[0], "skip");
    state = next(state);
    assert_eq!(part(&state).stitches, 7);
    let mut overflow = created();
    overflow = change(overflow, json!({"type":"adjust","stitches":"1000000"})).unwrap();
    overflow = plan(overflow);
    let id = part(&overflow).reminders[0].id.clone();
    overflow = choose(overflow, &id, "execute");
    assert_eq!(
        change(overflow, json!({"type":"complete","expectedLine":1}))
            .unwrap_err()
            .code,
        "knitting.stitches"
    );
}

#[test]
fn parts_and_projects_keep_independent_progress_and_selection() {
    let mut state = next(created());
    let first = part(&state).id.clone();
    state = change(
        state,
        json!({"type":"newPart","name":"Sleeve","kind":"rounds","stitches":"24"}),
    )
    .unwrap();
    let second = part(&state).id.clone();
    state = next(next(state));
    assert_eq!(inspect(&state).kind, Kind::Rounds);
    state = change(state, json!({"type":"selectPart","id":first})).unwrap();
    assert_eq!(part(&state).completed, 1);
    let project = state.active_project_id.clone().unwrap();
    state = change(
        state,
        json!({"type":"newProject","name":"Other","partName":"Piece","kind":"rows","stitches":"0"}),
    )
    .unwrap();
    state = change(state, json!({"type":"selectProject","id":project})).unwrap();
    assert_eq!(part(&state).completed, 1);
    state = change(state, json!({"type":"selectPart","id":second})).unwrap();
    assert_eq!((part(&state).completed, part(&state).stitches), (2, 24));
}

#[test]
fn copied_rules_start_from_zero_with_fresh_ids_and_initial_stitches() {
    let mut state = created();
    state = change(state, json!({"type":"adjust","stitches":"42"})).unwrap();
    state = plan(state);
    state = change(
        state,
        json!({"type":"addSegment","name":"Rib","rows":"4","repeats":"8"}),
    )
    .unwrap();
    state=change(state,json!({"type":"details","projectName":"Scarf","partName":"Front","note":"My own instructions"})).unwrap();
    let rule = part(&state).reminders[0].id.clone();
    state = choose(state, &rule, "execute");
    state = next(state);
    let source = part(&state).clone();
    state = change(state, json!({"type":"copyPart","name":"Back"})).unwrap();
    let copy = part(&state);
    assert_eq!(
        (copy.completed, copy.stitches, copy.initial_stitches),
        (0, 42, 42)
    );
    assert!(copy.undo.is_empty());
    assert!(copy.pending.is_empty());
    assert_ne!(copy.id, source.id);
    assert_ne!(copy.reminders[0].id, rule);
    assert_ne!(copy.segments[0].id, source.segments[0].id);
    assert_eq!(copy.note, source.note);
    assert_eq!(copy.reminders[0].start, source.reminders[0].start);
    assert_eq!(
        (
            state.projects[0].parts[0].completed,
            state.projects[0].parts[0].stitches
        ),
        (1, 44)
    );
}

#[test]
fn future_rules_never_rewrite_completed_pattern_rows() {
    let mut state = created();
    for _ in 0..5 {
        state = next(state);
    }
    assert_eq!(change(state.clone(),json!({"type":"addReminder","name":"late","start":"5","interval":"2","repeats":"1","delta":"1"})).unwrap_err().code,"knitting.past_rule");
    state = change(
        state,
        json!({"type":"addSegment","name":"New","rows":"2","repeats":"2"}),
    )
    .unwrap();
    assert_eq!(part(&state).segments[0].start, 6);
    assert_eq!(inspect(&state).pattern.unwrap().row, 1);
    let segment = part(&state).segments[0].id.clone();
    assert_eq!(
        change(state, json!({"type":"removeSegment","id":segment}))
            .unwrap_err()
            .code,
        "knitting.edit_locked"
    );
}

#[test]
fn clearing_and_deletion_require_confirmation_and_never_affect_other_parts() {
    let mut state = next(created());
    let first = part(&state).id.clone();
    state = change(
        state,
        json!({"type":"newPart","name":"Other","kind":"rows","stitches":"10"}),
    )
    .unwrap();
    state = next(state);
    for op in [
        json!({"type":"clear","confirm":false}),
        json!({"type":"deletePart","confirm":false}),
        json!({"type":"deleteProject","confirm":false}),
    ] {
        assert_eq!(
            change(state.clone(), op).unwrap_err().code,
            "knitting.confirmation"
        );
    }
    state = change(state, json!({"type":"clear","confirm":true})).unwrap();
    assert_eq!((part(&state).completed, part(&state).stitches), (0, 10));
    assert!(part(&state).undo.is_empty());
    state = change(state, json!({"type":"deletePart","confirm":true})).unwrap();
    assert_eq!(part(&state).id, first);
    assert_eq!(part(&state).completed, 1);
    state = change(state, json!({"type":"deletePart","confirm":true})).unwrap();
    assert!(state.active().is_none());
    assert_eq!(state.projects.len(), 1);
    state = change(state, json!({"type":"deleteProject","confirm":true})).unwrap();
    assert!(state.projects.is_empty());
    assert_eq!(state.active_project_id, None);
}

#[test]
fn stale_completion_and_bad_numbers_never_advance_progress() {
    let state = next(created());
    assert_eq!(
        change(state.clone(), json!({"type":"complete","expectedLine":1}))
            .unwrap_err()
            .code,
        "knitting.stale_line"
    );
    for bad in ["0", "-1", "1.5", "1e2", "4294967296"] {
        let op = json!({"type":"addSegment","name":"Rib","rows":bad,"repeats":"1"});
        assert!(change(state.clone(), op).is_err());
        let op = json!({"type":"addReminder","name":"Shape","start":"2","interval":bad,"repeats":"1","delta":"1"});
        assert!(change(state.clone(), op).is_err());
    }
    for bad in ["1000001", "-1", "1.5", "x"] {
        assert!(change(state.clone(), json!({"type":"adjust","stitches":bad})).is_err());
    }
    assert!(
        change(
            state.clone(),
            json!({"type":"addSegment","name":"big","rows":"100000","repeats":"10000"})
        )
        .is_err()
    );
    assert!(change(state,json!({"type":"addReminder","name":"big","start":"100000","interval":"100000","repeats":"10000","delta":"1"})).is_err());
}

#[test]
fn manual_correction_is_confirmed_and_undoable_without_advancing_a_row() {
    let mut state = created();
    state = change(state, json!({"type":"adjust","stitches":"35"})).unwrap();
    assert_eq!(
        (part(&state).completed, part(&state).initial_stitches),
        (0, 35)
    );
    state = change(state, json!({"type":"undo"})).unwrap();
    assert_eq!(
        (part(&state).stitches, part(&state).initial_stitches),
        (40, 40)
    );
    state = next(state);
    state = change(state, json!({"type":"adjust","stitches":"38"})).unwrap();
    assert_eq!(
        (part(&state).completed, part(&state).initial_stitches),
        (1, 40)
    );
    state = change(state, json!({"type":"undo"})).unwrap();
    assert_eq!((part(&state).completed, part(&state).stitches), (1, 40));
}

#[test]
fn malformed_saved_selection_and_zero_interval_are_rejected() {
    let mut state = created();
    state.active_project_id = Some("missing".into());
    assert!(validate(&state, &Context::default()).is_err());
    let mut state = plan(created());
    state.part_mut().unwrap().reminders[0].interval = 0;
    assert!(validate(&state, &Context::default()).is_err());
    let mut state = created();
    state.projects[0].active_part_id = None;
    assert!(validate(&state, &Context::default()).is_err());
    let mut state = created();
    state.schema_version = 2;
    assert_eq!(
        validate(&state, &Context::default()).unwrap_err().code,
        "knitting.version"
    );
    let mut value: Value = serde_json::to_value(created()).unwrap();
    value["revision"] = json!(-1);
    assert!(serde_json::from_value::<Workshop>(value).is_err());
}

#[test]
fn the_last_allowed_row_can_be_confirmed_and_undone_without_overflow() {
    let mut state = created();
    state.part_mut().unwrap().completed = MAX_ROWS - 1;
    state = next(state);
    let view = inspect(&state);
    assert_eq!(view.completed, MAX_ROWS);
    assert_eq!(view.next_line, MAX_ROWS + 1);
    assert!(!view.can_complete);
    assert_eq!(
        change(
            state.clone(),
            json!({"type":"complete","expectedLine":MAX_ROWS+1})
        )
        .unwrap_err()
        .code,
        "knitting.row_limit"
    );
    state = change(state, json!({"type":"undo"})).unwrap();
    assert_eq!(part(&state).completed, MAX_ROWS - 1);
}
