use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

pub const MAX_SCORE: i64 = 1_000_000;
pub const MAX_ROUNDS: usize = 200;
pub const MAX_GAMES: usize = 100;
pub const MAX_EVENTS: usize = 1000;
pub const MAX_STATE: usize = 2 * 1024 * 1024;
pub const MAX_REVISION: u64 = 9_007_199_254_740_991;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("boardgame-score.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(tf_plugin_api::cancelled())
    } else {
        Ok(())
    }
}
pub fn label(text: &str) -> PluginResult<String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 80 || text.chars().any(char::is_control) {
        return Err(error("name"));
    }
    Ok(text.to_owned())
}
pub fn integer(text: &str, limit: i64) -> PluginResult<i64> {
    let text = text.trim();
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error("score").with("limit", limit));
    }
    let n = text
        .parse::<i64>()
        .map_err(|_| error("score").with("limit", limit))?;
    if !(-limit..=limit).contains(&n) {
        return Err(error("score").with("limit", limit));
    }
    Ok(n)
}
pub fn id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("{time}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    High,
    Low,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Player {
    pub seat: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameData {
    pub name: String,
    pub players: Vec<Player>,
    pub direction: Direction,
    pub round_limit: Option<usize>,
    pub steps: Vec<i64>,
    pub rounds: Vec<Vec<i64>>,
    pub draft: Vec<Option<i64>>,
    pub finished: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CellChange {
    pub round: usize,
    pub seat: u32,
    pub before: Option<i64>,
    pub after: Option<i64>,
    pub draft: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    pub sequence: usize,
    pub kind: String,
    pub changes: Vec<CellChange>,
    pub before_text: Option<String>,
    pub after_text: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Game {
    pub id: String,
    pub data: GameData,
    pub undo: Option<GameData>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub schema_version: u32,
    pub revision: u64,
    pub selected_id: Option<String>,
    pub games: Vec<Game>,
}
impl State {
    pub fn active(&self) -> PluginResult<&Game> {
        self.games
            .iter()
            .find(|g| Some(&g.id) == self.selected_id.as_ref())
            .ok_or_else(|| error("missing"))
    }
    pub fn active_mut(&mut self) -> PluginResult<&mut Game> {
        self.games
            .iter_mut()
            .find(|g| Some(&g.id) == self.selected_id.as_ref())
            .ok_or_else(|| error("missing"))
    }
}

fn validate_data(data: &GameData) -> PluginResult<()> {
    label(&data.name)?;
    let n = data.players.len();
    if !(2..=12).contains(&n) {
        return Err(error("players"));
    }
    for (index, player) in data.players.iter().enumerate() {
        label(&player.name)?;
        if player.seat != index as u32 + 1 {
            return Err(error("state"));
        }
    }
    if data.round_limit.is_some_and(|n| n == 0 || n > MAX_ROUNDS)
        || data.rounds.len() > data.round_limit.unwrap_or(MAX_ROUNDS)
        || data.steps.is_empty()
        || data.steps.len() > 4
        || data.steps.iter().any(|n| !(1..=MAX_SCORE).contains(n))
        || data.steps.iter().collect::<BTreeSet<_>>().len() != data.steps.len()
        || data.draft.len() != n
        || data
            .draft
            .iter()
            .flatten()
            .any(|v| !(-MAX_SCORE..=MAX_SCORE).contains(v))
        || data
            .rounds
            .iter()
            .any(|row| row.len() != n || row.iter().any(|v| !(-MAX_SCORE..=MAX_SCORE).contains(v)))
        || (data.finished && (data.rounds.is_empty() || data.draft.iter().any(Option::is_some)))
        || (data.rounds.len() == data.round_limit.unwrap_or(MAX_ROUNDS)
            && data.draft.iter().any(Option::is_some))
    {
        return Err(error("state"));
    }
    Ok(())
}
pub fn validate(state: &State, ctx: &dyn TaskContext) -> PluginResult<()> {
    check(ctx)?;
    if state.schema_version != 1 || state.revision > MAX_REVISION || state.games.len() > MAX_GAMES {
        return Err(error("state"));
    }
    let mut ids = BTreeSet::new();
    for game in &state.games {
        check(ctx)?;
        if game.id.is_empty()
            || game.id.len() > 100
            || !ids.insert(&game.id)
            || game.events.len() > MAX_EVENTS
        {
            return Err(error("state"));
        }
        validate_data(&game.data)?;
        if let Some(undo) = &game.undo {
            validate_data(undo)?;
            if undo.direction != game.data.direction
                || undo.round_limit != game.data.round_limit
                || undo.players.len() != game.data.players.len()
                || undo.steps != game.data.steps
            {
                return Err(error("state"));
            }
        }
        for (index, event) in game.events.iter().enumerate() {
            if event.sequence != index + 1
                || ![
                    "draft",
                    "confirmRound",
                    "editRound",
                    "rename",
                    "finish",
                    "reopen",
                    "undo",
                ]
                .contains(&event.kind.as_str())
                || event.changes.len() > 24
                || [&event.before_text, &event.after_text]
                    .into_iter()
                    .flatten()
                    .any(|v| v.chars().count() > 1200 || v.chars().any(char::is_control))
                || event.changes.iter().any(|c| {
                    c.round == 0
                        || c.round > MAX_ROUNDS
                        || c.seat == 0
                        || c.seat > game.data.players.len() as u32
                        || [c.before, c.after]
                            .iter()
                            .flatten()
                            .any(|v| !(-MAX_SCORE..=MAX_SCORE).contains(v))
                })
            {
                return Err(error("state"));
            }
        }
    }
    if state.games.is_empty() != state.selected_id.is_none()
        || state
            .selected_id
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
    {
        return Err(error("state"));
    }
    if serde_json::to_vec(state).map_err(|_| error("state"))?.len() > MAX_STATE {
        return Err(error("storage_limit"));
    }
    Ok(())
}
