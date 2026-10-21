use crate::maze::*;
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub rows: String,
    pub cols: String,
    pub entrance_side: Side,
    pub entrance_index: String,
    pub exit_side: Side,
    pub exit_index: String,
    pub seed: String,
}
impl Draft {
    fn resolve(self) -> PluginResult<Spec> {
        use std::{
            sync::atomic::{AtomicU64, Ordering},
            time::{SystemTime, UNIX_EPOCH},
        };
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let number = |text: &str| text.trim().parse::<usize>().map_err(|_| error("size"));
        let seed = if self.seed.is_empty() {
            format!(
                "{:x}-{:x}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| error("seed"))?
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )
        } else {
            self.seed
        };
        Ok(Spec {
            algorithm: ALGORITHM.into(),
            rows: number(&self.rows)?,
            cols: number(&self.cols)?,
            entrance: Port {
                side: self.entrance_side,
                index: number(&self.entrance_index)?,
            },
            exit: Port {
                side: self.exit_side,
                index: number(&self.exit_index)?,
            },
            seed,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Game {
    pub spec: Spec,
    pub id: String,
    pub history: Vec<usize>,
    pub hints: usize,
    pub hint_target: Option<usize>,
    pub revealed: bool,
    pub answer_visible: bool,
    pub show_trail: bool,
    pub completion: Option<Score>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Score {
    pub steps: usize,
    pub hints: usize,
    pub used_answer: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub schema_version: u32,
    pub revision: u32,
    pub game: Option<Game>,
}
impl State {
    pub fn validate(&self, ctx: &dyn TaskContext) -> PluginResult<Option<Maze>> {
        check(ctx)?;
        if self.schema_version != 1 || self.revision >= 1_000_000_000 {
            return Err(error("version"));
        }
        let Some(g) = &self.game else {
            return Ok(None);
        };
        let maze = generate(g.spec.clone(), ctx)?;
        if g.id != g.spec.id()
            || g.history.is_empty()
            || g.history.len() > MAX_STEPS + 1
            || g.history[0] != maze.start
            || g.hints > 100_000
            || (g.answer_visible && !g.revealed)
        {
            return Err(error("state"));
        }
        for pair in g.history.windows(2) {
            check(ctx)?;
            if pair[0] == maze.exit || !maze.legal(pair[0], pair[1]) {
                return Err(error("state"));
            }
        }
        if let Some(target) = g.hint_target {
            let at = *g.history.last().unwrap();
            if g.hints == 0 || maze.route(at, ctx)?.get(1).copied() != Some(target) {
                return Err(error("state"));
            }
        }
        let finished = g.history.last().copied() == Some(maze.exit);
        if finished != g.completion.is_some() {
            return Err(error("state"));
        }
        if let Some(score) = &g.completion
            && (score.steps != g.history.len() - 1
                || score.hints != g.hints
                || (score.used_answer && !g.revealed))
        {
            return Err(error("state"));
        }
        Ok(Some(maze))
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Op {
    New { draft: Draft, confirm: bool },
    ImportCode { code: String, confirm: bool },
    Move { side: Side },
    MoveTo { cell: usize },
    Undo,
    Restart { confirm: bool },
    Hint,
    ToggleAnswer,
    ToggleTrail,
    MarkAnswer,
}
pub fn apply(
    state: &mut State,
    maze: Option<&Maze>,
    op: Op,
    ctx: &dyn TaskContext,
) -> PluginResult<(bool, &'static str)> {
    check(ctx)?;
    let op = match op {
        Op::New { draft, confirm } => return replace(state, draft.resolve()?, confirm, ctx),
        Op::ImportCode { code, confirm } => {
            return replace(state, Spec::from_code(&code)?, confirm, ctx);
        }
        op => op,
    };
    let maze = maze.ok_or_else(|| error("no_game"))?;
    let g = state.game.as_mut().ok_or_else(|| error("no_game"))?;
    let at = *g.history.last().unwrap();
    match op {
        Op::Move { side } => move_to(g, maze, maze.adjacent(at, side)),
        Op::MoveTo { cell } => move_to(g, maze, Some(cell)),
        Op::Undo => {
            if g.history.len() == 1 {
                return Ok((false, "blocked"));
            }
            g.history.pop();
            g.hint_target = None;
            g.completion = None;
            Ok((true, "undo"))
        }
        Op::Restart { confirm } => {
            if !confirm {
                return Err(error("confirm"));
            }
            g.history = vec![maze.start];
            g.hints = 0;
            g.hint_target = None;
            g.revealed = false;
            g.answer_visible = false;
            g.completion = None;
            Ok((true, "restart"))
        }
        Op::Hint => {
            let route = maze.route(at, ctx)?;
            let Some(&next) = route.get(1) else {
                return Ok((false, "finished"));
            };
            if g.hints >= 100_000 {
                return Err(error("limits"));
            }
            g.hints += 1;
            g.hint_target = Some(next);
            Ok((true, "hint"))
        }
        Op::ToggleAnswer => {
            g.answer_visible = !g.answer_visible;
            if g.answer_visible {
                g.revealed = true;
            }
            Ok((true, "answer"))
        }
        Op::ToggleTrail => {
            g.show_trail = !g.show_trail;
            Ok((true, "trail"))
        }
        Op::MarkAnswer => {
            g.revealed = true;
            Ok((true, "answer"))
        }
        Op::New { .. } | Op::ImportCode { .. } => unreachable!(),
    }
}
fn move_to(g: &mut Game, maze: &Maze, target: Option<usize>) -> PluginResult<(bool, &'static str)> {
    let at = *g.history.last().unwrap();
    if at == maze.exit {
        return Ok((false, "finished"));
    }
    let Some(target) = target.filter(|&to| maze.legal(at, to)) else {
        return Ok((false, "blocked"));
    };
    if g.history.len() > MAX_STEPS {
        return Err(error("limits"));
    }
    g.history.push(target);
    g.hint_target = None;
    if target == maze.exit {
        g.completion = Some(Score {
            steps: g.history.len() - 1,
            hints: g.hints,
            used_answer: g.revealed,
        });
    }
    Ok((
        true,
        if target == maze.exit {
            "finished"
        } else {
            "moved"
        },
    ))
}

fn replace(
    state: &mut State,
    spec: Spec,
    confirm: bool,
    ctx: &dyn TaskContext,
) -> PluginResult<(bool, &'static str)> {
    if state.game.is_some() && !confirm {
        return Err(error("confirm"));
    }
    let maze = generate(spec, ctx)?;
    state.game = Some(Game {
        spec: maze.spec.clone(),
        id: maze.spec.id(),
        history: vec![maze.start],
        hints: 0,
        hint_target: None,
        revealed: false,
        answer_visible: false,
        show_trail: true,
        completion: None,
    });
    Ok((true, "new"))
}

#[cfg(test)]
#[path = "game_test.rs"]
mod tests;
