use super::*;
use crate::tests::{Context, created, spec};

#[test]
fn hints_are_legal_next_steps_and_never_move_the_player() {
    let ctx = Context::default();
    let mut state = created();
    let maze = state.validate(&ctx).unwrap().unwrap();
    let old = state.game.as_ref().unwrap().history.clone();
    apply(&mut state, Some(&maze), Op::Hint, &ctx).unwrap();
    let g = state.game.as_ref().unwrap();
    let at = *g.history.last().unwrap();
    let next = g.hint_target.unwrap();
    assert!(maze.legal(at, next));
    assert_eq!(
        maze.route(at, &ctx).unwrap().len(),
        maze.route(next, &ctx).unwrap().len() + 1
    );
    assert_eq!(g.history, old);
    assert_eq!(g.hints, 1);
}
#[test]
fn wall_diagonal_teleport_and_outside_moves_do_not_change_progress() {
    let ctx = Context::default();
    let mut state = created();
    let maze = state.validate(&ctx).unwrap().unwrap();
    for cell in [maze.start, maze.start + maze.spec.cols + 1, usize::MAX] {
        let before = serde_json::to_value(&state).unwrap();
        let (changed, notice) = apply(&mut state, Some(&maze), Op::MoveTo { cell }, &ctx).unwrap();
        assert!(!changed);
        assert_eq!(notice, "blocked");
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
    }
    let mut tested = false;
    for side in SIDES {
        if let Some(cell) = maze.adjacent(maze.start, side)
            && !maze.legal(maze.start, cell)
        {
            tested = true;
            assert!(
                !apply(&mut state, Some(&maze), Op::MoveTo { cell }, &ctx)
                    .unwrap()
                    .0
            );
        }
    }
    assert!(tested);
}
#[test]
fn undo_preserves_assistance_and_restart_requires_confirmation() {
    let ctx = Context::default();
    let mut state = created();
    let maze = state.validate(&ctx).unwrap().unwrap();
    apply(&mut state, Some(&maze), Op::Hint, &ctx).unwrap();
    let cell = state.game.as_ref().unwrap().hint_target.unwrap();
    apply(&mut state, Some(&maze), Op::MoveTo { cell }, &ctx).unwrap();
    apply(&mut state, Some(&maze), Op::ToggleAnswer, &ctx).unwrap();
    apply(&mut state, Some(&maze), Op::ToggleAnswer, &ctx).unwrap();
    apply(&mut state, Some(&maze), Op::Undo, &ctx).unwrap();
    let g = state.game.as_ref().unwrap();
    assert_eq!(g.history.len(), 1);
    assert_eq!(g.hints, 1);
    assert!(g.revealed);
    assert!(!g.answer_visible);
    assert!(g.hint_target.is_none());
    assert!(
        apply(
            &mut state,
            Some(&maze),
            Op::Restart { confirm: false },
            &ctx
        )
        .is_err()
    );
    apply(&mut state, Some(&maze), Op::Restart { confirm: true }, &ctx).unwrap();
    let g = state.game.unwrap();
    assert!(!g.revealed);
    assert_eq!(g.hints, 0);
    assert_eq!(g.history, vec![maze.start]);
}
#[test]
fn completion_captures_assistance_at_finish_and_no_move_follows_exit() {
    let ctx = Context::default();
    let mut state = created();
    let maze = state.validate(&ctx).unwrap().unwrap();
    let route = maze.route(maze.start, &ctx).unwrap();
    for &cell in &route[1..] {
        apply(&mut state, Some(&maze), Op::MoveTo { cell }, &ctx).unwrap();
    }
    let score = state.game.as_ref().unwrap().completion.as_ref().unwrap();
    assert_eq!(score.steps, route.len() - 1);
    assert!(!score.used_answer);
    assert_eq!(score.hints, 0);
    assert!(
        !apply(
            &mut state,
            Some(&maze),
            Op::MoveTo {
                cell: route[route.len() - 2]
            },
            &ctx
        )
        .unwrap()
        .0
    );
    apply(&mut state, Some(&maze), Op::MarkAnswer, &ctx).unwrap();
    assert!(
        !state
            .game
            .as_ref()
            .unwrap()
            .completion
            .as_ref()
            .unwrap()
            .used_answer
    );
    apply(&mut state, Some(&maze), Op::Undo, &ctx).unwrap();
    assert!(state.game.as_ref().unwrap().completion.is_none());
    apply(
        &mut state,
        Some(&maze),
        Op::MoveTo { cell: maze.exit },
        &ctx,
    )
    .unwrap();
    assert!(
        state
            .game
            .as_ref()
            .unwrap()
            .completion
            .as_ref()
            .unwrap()
            .used_answer
    );
    state.validate(&ctx).unwrap();
}
#[test]
fn replacing_a_puzzle_resets_progress_and_cannot_carry_old_paths() {
    let ctx = Context::default();
    let mut state = created();
    let maze = state.validate(&ctx).unwrap().unwrap();
    apply(&mut state, Some(&maze), Op::Hint, &ctx).unwrap();
    let code = spec("different").code();
    assert!(
        apply(
            &mut state,
            Some(&maze),
            Op::ImportCode {
                code: code.clone(),
                confirm: false
            },
            &ctx
        )
        .is_err()
    );
    apply(
        &mut state,
        Some(&maze),
        Op::ImportCode {
            code,
            confirm: true,
        },
        &ctx,
    )
    .unwrap();
    let g = state.game.as_ref().unwrap();
    assert_ne!(g.id, maze.spec.id());
    assert_eq!(g.history.len(), 1);
    assert_eq!(g.hints, 0);
    assert!(g.hint_target.is_none());
    let mut tampered = state.clone();
    tampered.game.as_mut().unwrap().spec.seed.push('x');
    assert!(tampered.validate(&ctx).is_err());
}
#[test]
fn invalid_saved_paths_hint_targets_and_step_limits_fail_without_trimming() {
    let ctx = Context::default();
    let state = created();
    let mut bad = state.clone();
    bad.game.as_mut().unwrap().history.push(99999);
    assert!(bad.validate(&ctx).is_err());
    let mut bad = state.clone();
    bad.game.as_mut().unwrap().hint_target = Some(99999);
    assert!(bad.validate(&ctx).is_err());
    let mut bad = state;
    bad.game.as_mut().unwrap().history = vec![0; MAX_STEPS + 2];
    assert!(bad.validate(&ctx).is_err());
    assert_eq!(bad.game.unwrap().history.len(), MAX_STEPS + 2);
}
