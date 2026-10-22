use super::*;
use crate::model::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tf_plugin_api::LogLevel;

#[derive(Default)]
struct Context {
    cancelled: AtomicBool,
    cancel_on: Option<&'static str>,
}
impl TaskContext for Context {
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Err(error("file"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(error("file"))
    }
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, code: &str) {
        if self.cancel_on == Some(code) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
fn empty() -> State {
    State {
        schema_version: 1,
        ..State::default()
    }
}
fn apply(state: State, op: Value) -> PluginResult<State> {
    ops::apply(
        state,
        serde_json::from_value(op).unwrap(),
        &Context::default(),
    )
}
fn created(direction: &str, limit: &str) -> State {
    apply(empty(),json!({"type":"new","name":"Friday","players":["Alex","Alex","Bo"],"direction":direction,"roundLimit":limit,"steps":["1","5"]})).unwrap()
}
fn round(state: State, values: Vec<&str>) -> State {
    let expected = state.active().unwrap().data.rounds.len() + 1;
    apply(
        state,
        json!({"type":"confirmRound","values":values,"expectedRound":expected}),
    )
    .unwrap()
}

#[test]
fn scores_distinguish_missing_zero_and_negatives_with_strict_bounds() {
    let state = created("high", "");
    assert!(
        apply(
            state.clone(),
            json!({"type":"confirmRound","values":["0","","-7"],"expectedRound":1})
        )
        .is_err()
    );
    for bad in [
        "1.0",
        "1e2",
        "NaN",
        "1000001",
        "-1000001",
        "9999999999999999999999",
        "++1",
        "+-1",
    ] {
        assert!(
            apply(
                state.clone(),
                json!({"type":"draft","values":[bad,"0","-7"],"expectedRound":1})
            )
            .is_err(),
            "{bad}"
        );
    }
    let saved = apply(
        state,
        json!({"type":"draft","values":["0","","-7"],"expectedRound":1}),
    )
    .unwrap();
    assert_eq!(
        saved.active().unwrap().data.draft,
        vec![Some(0), None, Some(-7)]
    );
    assert!(
        view::rankings(&saved.active().unwrap().data)
            .iter()
            .all(|r| r.total == 0 && !r.leading)
    );
    let confirmed = round(saved, vec!["+0", "1000000", "-1000000"]);
    assert_eq!(
        confirmed.active().unwrap().data.rounds[0],
        vec![0, 1000000, -1000000]
    );
}
#[test]
fn shared_competition_ranks_and_low_wins_never_break_ties_by_seat() {
    let state = round(created("high", ""), vec!["5", "5", "-1"]);
    let ranking = view::rankings(&state.active().unwrap().data);
    assert_eq!(
        ranking.iter().map(|r| r.rank).collect::<Vec<_>>(),
        vec![1, 1, 3]
    );
    assert_eq!(ranking.iter().filter(|r| r.leading).count(), 2);
    let low = round(created("low", ""), vec!["-10", "3", "-10"]);
    assert_eq!(
        view::rankings(&low.active().unwrap().data)
            .iter()
            .map(|r| (r.seat, r.rank))
            .collect::<Vec<_>>(),
        vec![(1, 1), (3, 1), (2, 3)]
    );
}
#[test]
fn correcting_old_rounds_recalculates_later_cumulative_and_undo_restores() {
    let state = round(
        round(created("high", ""), vec!["2", "3", "4"]),
        vec!["1", "0", "-2"],
    );
    let edited = apply(
        state.clone(),
        json!({"type":"editRound","round":1,"values":["9","3","4"]}),
    )
    .unwrap();
    assert_eq!(
        view::rows(&edited.active().unwrap().data)[1].cumulative,
        vec![10, 3, 2]
    );
    let event = edited.active().unwrap().events.last().unwrap();
    assert_eq!(
        (event.changes[0].before, event.changes[0].after),
        (Some(2), Some(9))
    );
    let undone = apply(edited, json!({"type":"undo"})).unwrap();
    assert_eq!(undone.active().unwrap().data, state.active().unwrap().data);
    assert!(apply(undone, json!({"type":"undo"})).is_err());
}
#[test]
fn duplicate_confirmation_cannot_add_a_round_or_duplicate_settlement() {
    let state = round(created("high", "1"), vec!["0", "0", "0"]);
    assert!(!state.active().unwrap().data.finished);
    assert!(
        apply(
            state.clone(),
            json!({"type":"confirmRound","expectedRound":1,"values":["1","1","1"]})
        )
        .is_err()
    );
    assert!(
        apply(
            state.clone(),
            json!({"type":"confirmRound","expectedRound":2,"values":["1","1","1"]})
        )
        .is_err()
    );
    assert!(apply(state.clone(), json!({"type":"finish","confirm":false})).is_err());
    let finished = apply(state, json!({"type":"finish","confirm":true})).unwrap();
    assert_eq!(
        view::rankings(&finished.active().unwrap().data)
            .iter()
            .filter(|r| r.rank == 1)
            .count(),
        3
    );
    assert!(apply(finished, json!({"type":"finish","confirm":true})).is_err());
}
#[test]
fn finished_games_require_explicit_reopening_and_new_settlement() {
    let state = round(created("high", ""), vec!["1", "2", "3"]);
    let finished = apply(state, json!({"type":"finish","confirm":true})).unwrap();
    assert!(
        apply(
            finished.clone(),
            json!({"type":"editRound","round":1,"values":["9","2","3"]})
        )
        .is_err()
    );
    assert!(apply(finished.clone(), json!({"type":"reopen","confirm":false})).is_err());
    let reopened = apply(finished, json!({"type":"reopen","confirm":true})).unwrap();
    let changed = apply(
        reopened,
        json!({"type":"editRound","round":1,"values":["9","2","3"]}),
    )
    .unwrap();
    assert!(!changed.active().unwrap().data.finished);
    assert_eq!(view::rankings(&changed.active().unwrap().data)[0].seat, 1);
    let finished = apply(changed, json!({"type":"finish","confirm":true})).unwrap();
    assert!(finished.active().unwrap().data.finished);
}
#[test]
fn pending_drafts_block_settlement_and_can_be_saved_or_undone() {
    let state = round(created("high", ""), vec!["1", "2", "3"]);
    let draft = apply(
        state.clone(),
        json!({"type":"adjust","values":["","",""],"expectedRound":2,"seat":2,"delta":"-5"}),
    )
    .unwrap();
    assert_eq!(draft.active().unwrap().data.draft[1], Some(-5));
    assert!(apply(draft.clone(), json!({"type":"finish","confirm":true})).is_err());
    let undo = apply(draft, json!({"type":"undo"})).unwrap();
    assert_eq!(undo.active().unwrap().data, state.active().unwrap().data);
    assert!(apply(state,json!({"type":"adjust","values":["1000000","",""],"expectedRound":2,"seat":1,"delta":"1"})).is_err());
}
#[test]
fn renaming_duplicate_players_preserves_identity_and_copy_starts_fresh() {
    let state = round(created("high", "6"), vec!["9", "2", "3"]);
    let state = apply(
        state,
        json!({"type":"rename","name":"New title","players":["Sam","Sam","Bo"]}),
    )
    .unwrap();
    assert_eq!(view::rankings(&state.active().unwrap().data)[0].seat, 1);
    let id = state.active().unwrap().id.clone();
    let copied = apply(state, json!({"type":"copy","name":"Next game"})).unwrap();
    let game = copied.active().unwrap();
    assert_ne!(game.id, id);
    assert!(game.data.rounds.is_empty());
    assert!(game.events.is_empty());
    assert_eq!(game.data.round_limit, Some(6));
    assert_eq!(game.data.players[0].name, "Sam");
    assert!(game.data.draft.iter().all(Option::is_none));
    let selected = apply(copied, json!({"type":"select","id":id})).unwrap();
    assert_eq!(selected.active().unwrap().data.rounds[0][0], 9);
}
#[test]
fn sqlite_payload_roundtrips_drafts_undo_and_history_without_recalculated_storage() {
    let state = round(created("high", ""), vec!["1", "2", "3"]);
    let state = apply(
        state,
        json!({"type":"draft","values":["-2","0",""],"expectedRound":2}),
    )
    .unwrap();
    let saved = serde_json::to_vec(&state).unwrap();
    let restored: State = serde_json::from_slice(&saved).unwrap();
    validate(&restored, &Context::default()).unwrap();
    assert_eq!(
        restored.active().unwrap().data.draft,
        vec![Some(-2), Some(0), None]
    );
    let undone = apply(restored, json!({"type":"undo"})).unwrap();
    assert_eq!(undone.active().unwrap().data.rounds.len(), 1);
}
#[test]
fn limits_reject_without_truncation_and_extreme_totals_remain_exact() {
    for count in [1, 13] {
        assert!(apply(empty(),json!({"type":"new","name":"X","players":vec!["P";count],"direction":"high","roundLimit":"","steps":["1"]})).is_err());
    }
    for limit in ["0", "201", "-1"] {
        assert!(apply(empty(),json!({"type":"new","name":"X","players":["A","B"],"direction":"high","roundLimit":limit,"steps":["1"]})).is_err());
    }
    let mut state = created("high", "");
    let game = state.active_mut().unwrap();
    game.data.rounds = vec![vec![MAX_SCORE, -MAX_SCORE, 0]; MAX_ROUNDS];
    validate(&state, &Context::default()).unwrap();
    assert_eq!(
        view::rankings(&state.active().unwrap().data)[0].total,
        200_000_000
    );
    assert!(
        apply(
            state,
            json!({"type":"confirmRound","values":["0","0","0"],"expectedRound":201})
        )
        .is_err()
    );
    let mut state = created("high", "");
    state.active_mut().unwrap().data.players[0].seat = 2;
    assert!(validate(&state, &Context::default()).is_err());
}
#[test]
fn audit_limit_blocks_instead_of_losing_user_history_and_delete_is_explicit() {
    let mut state = created("high", "");
    let game = state.active_mut().unwrap();
    game.events = (0..MAX_EVENTS)
        .map(|i| Event {
            sequence: i + 1,
            kind: "draft".into(),
            changes: vec![],
            before_text: None,
            after_text: None,
        })
        .collect();
    assert!(
        apply(
            state.clone(),
            json!({"type":"draft","values":["0","",""],"expectedRound":1})
        )
        .is_err()
    );
    assert!(apply(state.clone(), json!({"type":"delete","confirm":false})).is_err());
    let deleted = apply(state, json!({"type":"delete","confirm":true})).unwrap();
    assert!(deleted.games.is_empty());
    assert!(deleted.selected_id.is_none());
}
#[test]
fn pages_are_backend_bounded_and_do_not_hide_the_selected_game() {
    let mut state = created("high", "");
    let first = state.active().unwrap().id.clone();
    for _ in 0..10 {
        state = apply(state, json!({"type":"copy","name":"Copy"})).unwrap();
    }
    state = apply(state, json!({"type":"select","id":first})).unwrap();
    let out = view::inspect(&state, view::Pages::default(), true, &Context::default()).unwrap();
    assert_eq!(out["games"]["offset"], 8);
    assert_eq!(out["games"]["total"], 11);
    assert_eq!(out["games"]["items"].as_array().unwrap().len(), 3);
    for _ in 0..12 {
        state = round(state, vec!["1", "2", "3"]);
    }
    let out = view::inspect(
        &state,
        view::Pages {
            games: usize::MAX,
            rounds: usize::MAX,
            events: usize::MAX,
        },
        false,
        &Context::default(),
    )
    .unwrap();
    assert_eq!(out["active"]["rounds"]["offset"], 10);
    assert_eq!(out["active"]["rounds"]["items"][1]["cumulative"][2], 36);
}
#[test]
fn task_protocol_is_camel_case_cancellable_and_returns_atomic_revision() {
    let plugin = BoardgameScore::default();
    let state = created("high", "");
    let args = json!({"state":state,"requestId":"r1","op":{"type":"confirmRound","expectedRound":1,"values":["0","-2","1"]}});
    let out = plugin
        .run_task("apply", args.clone(), &Context::default())
        .unwrap();
    assert_eq!(out["baseRevision"], 1);
    assert_eq!(out["state"]["revision"], 2);
    assert_eq!(out["view"]["active"]["confirmed"], 1);
    assert_eq!(out["view"]["active"]["nextRound"], 2);
    let ctx = Context {
        cancel_on: Some("view"),
        ..Context::default()
    };
    assert!(plugin.run_task("apply", args, &ctx).is_err());
    assert_eq!(state.active().unwrap().data.rounds.len(), 0);
    assert!(plugin.call("apply", json!({})).is_err());
    assert!(
        plugin
            .run_task("missing", json!({}), &Context::default())
            .is_err()
    );
}
#[test]
fn exports_quote_formula_names_preserve_negatives_and_leave_drafts_out_of_csv() {
    let state = round(created("high", ""), vec!["-3", "0", "1"]);
    let state = apply(
        state,
        json!({"type":"rename","name":"=SUM(1,2)","players":["@Alex","Alex","Bo"]}),
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!("tf-boardgame-{}", id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scores.csv");
    let args =
        json!({"state":state,"format":"csv","language":"en-US","path":path.to_str().unwrap()});
    export::save(serde_json::from_value(args).unwrap(), &Context::default()).unwrap();
    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(csv.contains("\"'=SUM(1,2)\""));
    assert!(csv.contains("\"'@Alex\""));
    assert!(csv.contains(",-3,-3,3,"));
    assert_eq!(csv.lines().count(), 4);
    let out = export::save(
        serde_json::from_value(
            json!({"state":state,"format":"text","language":"zh-CN","path":null}),
        )
        .unwrap(),
        &Context::default(),
    )
    .unwrap();
    assert!(out["content"].as_str().unwrap().contains("共同名次"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fixed_round_target_and_host_sqlite_size_are_enforced_before_persistence() {
    let state = round(created("high", "2"), vec!["1", "2", "3"]);
    assert!(apply(state.clone(), json!({"type":"finish","confirm":true})).is_err());
    let view = view::inspect(&state, view::Pages::default(), false, &Context::default()).unwrap();
    assert_eq!(view["active"]["canFinish"], false);
    let state = round(state, vec!["1", "2", "3"]);
    assert!(apply(state, json!({"type":"finish","confirm":true})).is_ok());
    let mut state = created("high", "");
    let mut game = state.active().unwrap().clone();
    game.events = (0..MAX_EVENTS)
        .map(|i| Event {
            sequence: i + 1,
            kind: "rename".into(),
            changes: vec![],
            before_text: Some("a".repeat(1200)),
            after_text: Some("b".repeat(1200)),
        })
        .collect();
    state.games[0] = game;
    assert_eq!(
        validate(&state, &Context::default()).unwrap_err().code,
        "boardgame-score.storage_limit"
    );
    assert_eq!(MAX_STATE, 2 * 1024 * 1024);
}
