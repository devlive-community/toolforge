use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pages {
    #[serde(default)]
    pub games: usize,
    #[serde(default)]
    pub rounds: usize,
    #[serde(default)]
    pub events: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
}
pub fn page<T: Clone>(items: &[T], offset: usize, limit: usize) -> Page<T> {
    let offset = offset.min(items.len().saturating_sub(1)) / limit * limit;
    Page {
        items: items.iter().skip(offset).take(limit).cloned().collect(),
        offset,
        limit,
        total: items.len(),
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ranking {
    pub seat: u32,
    pub name: String,
    pub total: i64,
    pub rank: usize,
    pub leading: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub number: usize,
    pub scores: Vec<i64>,
    pub cumulative: Vec<i64>,
}
pub fn rows(data: &GameData) -> Vec<Round> {
    let mut totals = vec![0; data.players.len()];
    data.rounds
        .iter()
        .enumerate()
        .map(|(index, scores)| {
            for (total, score) in totals.iter_mut().zip(scores) {
                *total += score;
            }
            Round {
                number: index + 1,
                scores: scores.clone(),
                cumulative: totals.clone(),
            }
        })
        .collect()
}
pub fn rankings(data: &GameData) -> Vec<Ranking> {
    let totals = rows(data)
        .last()
        .map(|r| r.cumulative.clone())
        .unwrap_or_else(|| vec![0; data.players.len()]);
    let mut result: Vec<_> = data
        .players
        .iter()
        .zip(totals)
        .map(|(p, total)| Ranking {
            seat: p.seat,
            name: p.name.clone(),
            total,
            rank: 0,
            leading: false,
        })
        .collect();
    result.sort_by(|a, b| match data.direction {
        Direction::High => b.total.cmp(&a.total),
        Direction::Low => a.total.cmp(&b.total),
    });
    let mut rank = 1;
    let mut previous = None;
    for (i, row) in result.iter_mut().enumerate() {
        if previous.is_some_and(|v| v != row.total) {
            rank = i + 1;
        }
        row.rank = rank;
        row.leading = rank == 1 && !data.rounds.is_empty();
        previous = Some(row.total);
    }
    result
}
pub fn inspect(
    state: &State,
    pages: Pages,
    follow: bool,
    ctx: &dyn TaskContext,
) -> PluginResult<Value> {
    check(ctx)?;
    let games: Vec<_> = state.games.iter().rev().map(|g| json!({"id":g.id,"name":g.data.name,"players":g.data.players.len(),"rounds":g.data.rounds.len(),"finished":g.data.finished})).collect();
    let offset = if follow {
        games
            .iter()
            .position(|g| g["id"].as_str() == state.selected_id.as_deref())
            .unwrap_or(0)
            / 8
            * 8
    } else {
        pages.games
    };
    let games_page = page(&games, offset, 8);
    let mut game_options = games_page.items.clone();
    if let Some(selected) = games
        .iter()
        .find(|g| g["id"].as_str() == state.selected_id.as_deref())
        && !game_options.iter().any(|g| g["id"] == selected["id"])
    {
        game_options.insert(0, selected.clone());
    }
    let games = games_page;
    let active = if let Ok(game) = state.active() {
        let data = &game.data;
        let rounds = rows(data);
        let round_offset = if follow {
            rounds.len().saturating_sub(1) / 10 * 10
        } else {
            pages.rounds
        };
        let events: Vec<_> = game.events.iter().rev().cloned().collect();
        Some(
            json!({"id":game.id,"name":data.name,"players":data.players,"direction":data.direction,
            "roundLimit":data.round_limit,"steps":data.steps,"draft":data.draft,"finished":data.finished,
            "confirmed":rounds.len(),"nextRound":rounds.len()+1,"canUndo":game.undo.is_some(),
            "canAdd":!data.finished && rounds.len()<data.round_limit.unwrap_or(MAX_ROUNDS),
            "canFinish":!data.finished && !rounds.is_empty() && data.draft.iter().all(Option::is_none)
                && data.round_limit.is_none_or(|n| rounds.len() == n),
            "ranking":rankings(data),"rounds":page(&rounds,round_offset,10),"events":page(&events, if follow {0} else {pages.events}, 10)}),
        )
    } else {
        None
    };
    Ok(json!({"games":games,"gameOptions":game_options,"active":active}))
}
