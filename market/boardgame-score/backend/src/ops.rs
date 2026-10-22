use crate::model::*;
use serde::Deserialize;
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Op {
    New {
        name: String,
        players: Vec<String>,
        direction: Direction,
        round_limit: String,
        steps: Vec<String>,
    },
    Copy {
        name: String,
    },
    Select {
        id: String,
    },
    Delete {
        confirm: bool,
    },
    Draft {
        values: Vec<String>,
        expected_round: usize,
    },
    Adjust {
        values: Vec<String>,
        expected_round: usize,
        seat: u32,
        delta: String,
    },
    ConfirmRound {
        values: Vec<String>,
        expected_round: usize,
    },
    EditRound {
        round: usize,
        values: Vec<String>,
    },
    Rename {
        name: String,
        players: Vec<String>,
    },
    Finish {
        confirm: bool,
    },
    Reopen {
        confirm: bool,
    },
    Undo,
}

fn scores(values: Vec<String>, count: usize, complete: bool) -> PluginResult<Vec<Option<i64>>> {
    if values.len() != count {
        return Err(error("players"));
    }
    values
        .iter()
        .map(|text| {
            if text.trim().is_empty() {
                if complete {
                    Err(error("incomplete"))
                } else {
                    Ok(None)
                }
            } else {
                integer(text, MAX_SCORE).map(Some)
            }
        })
        .collect()
}
fn next(data: &GameData, expected: usize) -> PluginResult<()> {
    if expected != data.rounds.len() + 1 {
        return Err(error("stale"));
    }
    if data.rounds.len() >= data.round_limit.unwrap_or(MAX_ROUNDS) {
        return Err(error("round_limit"));
    }
    Ok(())
}
fn text(data: &GameData) -> String {
    std::iter::once(data.name.clone())
        .chain(
            data.players
                .iter()
                .map(|p| format!("#{} {}", p.seat, p.name)),
        )
        .collect::<Vec<_>>()
        .join(" · ")
}
fn event(before: &GameData, after: &GameData, kind: &str, sequence: usize) -> Event {
    let mut changes = Vec::new();
    for row in 0..before.rounds.len().max(after.rounds.len()) {
        for (index, player) in before.players.iter().enumerate() {
            let a = before.rounds.get(row).map(|r| r[index]);
            let b = after.rounds.get(row).map(|r| r[index]);
            if a != b {
                changes.push(CellChange {
                    round: row + 1,
                    seat: player.seat,
                    before: a,
                    after: b,
                    draft: false,
                });
            }
        }
    }
    // Confirmation already records each score as an official round entry.
    if kind != "confirmRound" && before.rounds.len() == after.rounds.len() {
        for (index, player) in before.players.iter().enumerate() {
            if before.draft[index] != after.draft[index] {
                changes.push(CellChange {
                    round: before.rounds.len() + 1,
                    seat: player.seat,
                    before: before.draft[index],
                    after: after.draft[index],
                    draft: true,
                });
            }
        }
    }
    let renamed = before.name != after.name || before.players != after.players;
    Event {
        sequence,
        kind: kind.to_owned(),
        changes,
        before_text: renamed.then(|| text(before)),
        after_text: renamed.then(|| text(after)),
    }
}
fn add(state: &mut State, data: GameData) -> PluginResult<()> {
    if state.games.len() >= MAX_GAMES {
        return Err(error("games_limit"));
    }
    let id = id();
    state.selected_id = Some(id.clone());
    state.games.push(Game {
        id,
        data,
        undo: None,
        events: Vec::new(),
    });
    Ok(())
}

pub fn apply(mut state: State, op: Op, ctx: &dyn TaskContext) -> PluginResult<State> {
    match op {
        Op::New {
            name,
            players,
            direction,
            round_limit,
            steps,
        } => {
            let round_limit = if round_limit.trim().is_empty() {
                None
            } else {
                let n = integer(&round_limit, MAX_ROUNDS as i64)?;
                if n < 1 {
                    return Err(error("round_limit"));
                }
                Some(n as usize)
            };
            if !(2..=12).contains(&players.len()) {
                return Err(error("players"));
            }
            let players = players
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    Ok(Player {
                        seat: i as u32 + 1,
                        name: label(n)?,
                    })
                })
                .collect::<PluginResult<Vec<_>>>()?;
            let count = players.len();
            let steps: Vec<i64> = steps
                .iter()
                .map(|s| integer(s, MAX_SCORE))
                .collect::<PluginResult<_>>()?;
            if steps.is_empty()
                || steps.len() > 4
                || steps.iter().any(|n| !(1..=MAX_SCORE).contains(n))
                || steps
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != steps.len()
            {
                return Err(error("step"));
            }
            add(
                &mut state,
                GameData {
                    name: label(&name)?,
                    players,
                    direction,
                    round_limit,
                    steps,
                    rounds: Vec::new(),
                    draft: vec![None; count],
                    finished: false,
                },
            )?;
        }
        Op::Copy { name } => {
            let mut data = state.active()?.data.clone();
            data.name = label(&name)?;
            data.rounds.clear();
            data.draft.fill(None);
            data.finished = false;
            add(&mut state, data)?;
        }
        Op::Select { id } => {
            if !state.games.iter().any(|g| g.id == id) {
                return Err(error("missing"));
            }
            state.selected_id = Some(id);
        }
        Op::Delete { confirm } => {
            if !confirm {
                return Err(error("confirm"));
            }
            let id = state.active()?.id.clone();
            state.games.retain(|g| g.id != id);
            state.selected_id = state.games.last().map(|g| g.id.clone());
        }
        op => {
            let game = state.active_mut()?;
            let before = game.data.clone();
            if game.events.len() >= MAX_EVENTS {
                return Err(error("events_limit"));
            }
            if before.finished && !matches!(op, Op::Reopen { .. } | Op::Undo) {
                return Err(error("finished"));
            }
            let kind = match op {
                Op::Draft {
                    values,
                    expected_round,
                } => {
                    next(&before, expected_round)?;
                    game.data.draft = scores(values, before.players.len(), false)?;
                    "draft"
                }
                Op::Adjust {
                    values,
                    expected_round,
                    seat,
                    delta,
                } => {
                    next(&before, expected_round)?;
                    let delta = integer(&delta, MAX_SCORE)?;
                    if !before.steps.contains(&delta.abs())
                        || seat == 0
                        || seat > before.players.len() as u32
                    {
                        return Err(error("step"));
                    }
                    let mut draft = scores(values, before.players.len(), false)?;
                    let score = draft[seat as usize - 1]
                        .unwrap_or(0)
                        .checked_add(delta)
                        .ok_or_else(|| error("score"))?;
                    if !(-MAX_SCORE..=MAX_SCORE).contains(&score) {
                        return Err(error("score").with("limit", MAX_SCORE));
                    }
                    draft[seat as usize - 1] = Some(score);
                    game.data.draft = draft;
                    "draft"
                }
                Op::ConfirmRound {
                    values,
                    expected_round,
                } => {
                    next(&before, expected_round)?;
                    let row = scores(values, before.players.len(), true)?
                        .into_iter()
                        .map(Option::unwrap)
                        .collect();
                    game.data.rounds.push(row);
                    game.data.draft.fill(None);
                    "confirmRound"
                }
                Op::EditRound { round, values } => {
                    if round == 0 || round > before.rounds.len() {
                        return Err(error("round"));
                    }
                    game.data.rounds[round - 1] = scores(values, before.players.len(), true)?
                        .into_iter()
                        .map(Option::unwrap)
                        .collect();
                    "editRound"
                }
                Op::Rename { name, players } => {
                    if players.len() != before.players.len() {
                        return Err(error("players"));
                    }
                    game.data.name = label(&name)?;
                    for (player, name) in game.data.players.iter_mut().zip(players) {
                        player.name = label(&name)?;
                    }
                    "rename"
                }
                Op::Finish { confirm } => {
                    if !confirm {
                        return Err(error("confirm"));
                    }
                    if before.rounds.is_empty() || before.draft.iter().any(Option::is_some) {
                        return Err(error("unsettled"));
                    }
                    if before.round_limit.is_some_and(|n| before.rounds.len() != n) {
                        return Err(error("target"));
                    }
                    game.data.finished = true;
                    "finish"
                }
                Op::Reopen { confirm } => {
                    if !confirm {
                        return Err(error("confirm"));
                    }
                    if !before.finished {
                        return Err(error("active"));
                    }
                    game.data.finished = false;
                    "reopen"
                }
                Op::Undo => {
                    game.data = game.undo.take().ok_or_else(|| error("undo"))?;
                    "undo"
                }
                _ => unreachable!(),
            };
            if game.data == before {
                return Err(error("unchanged"));
            }
            game.events
                .push(event(&before, &game.data, kind, game.events.len() + 1));
            if kind != "undo" {
                game.undo = Some(before);
            }
        }
    }
    state.revision = state
        .revision
        .checked_add(1)
        .filter(|n| *n <= MAX_REVISION)
        .ok_or_else(|| error("storage_limit"))?;
    validate(&state, ctx)?;
    Ok(state)
}
