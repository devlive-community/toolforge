//! 命令行 / Agent 用的扁平入口：给出每位玩家各轮分数，算出总分与共同名次。
//!
//! 玩家写法：`名字:分/分/分`（各轮用 `/` 分隔，每人轮数必须相同）。排名复用界面同一套
//! 竞赛排名（同分并列，下一名次跳过，如 1、1、3），分数与名字校验也与界面一致。

use serde::{Deserialize, Serialize};
use tf_plugin_api::PluginResult;

use crate::model::{Direction, GameData, MAX_ROUNDS, MAX_SCORE, Player, error, integer, label};
use crate::view::rankings;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankArgs {
    pub players: Vec<String>,
    #[serde(default = "high")]
    pub direction: Direction,
}

fn high() -> Direction {
    Direction::High
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub rank: usize,
    pub name: String,
    pub total: i64,
}

pub fn rank(args: RankArgs) -> PluginResult<Vec<Row>> {
    if !(2..=12).contains(&args.players.len()) {
        return Err(error("players"));
    }
    let mut players = Vec::with_capacity(args.players.len());
    let mut scores: Vec<Vec<i64>> = Vec::with_capacity(args.players.len());
    for (seat, line) in args.players.iter().enumerate() {
        let (name, list) = line.rsplit_once(':').ok_or_else(|| error("incomplete"))?;
        players.push(Player {
            seat: seat as u32 + 1,
            name: label(name)?,
        });
        let row = list
            .split('/')
            .map(|s| integer(s, MAX_SCORE))
            .collect::<PluginResult<Vec<_>>>()?;
        if row.len() > MAX_ROUNDS {
            return Err(error("round_limit"));
        }
        scores.push(row);
    }
    let rounds = scores[0].len();
    if scores.iter().any(|r| r.len() != rounds) {
        return Err(error("incomplete"));
    }
    // 按轮转置：rounds[i][seat]
    let data = GameData {
        name: String::new(),
        draft: vec![None; players.len()],
        players,
        direction: args.direction,
        round_limit: None,
        steps: Vec::new(),
        rounds: (0..rounds)
            .map(|i| scores.iter().map(|r| r[i]).collect())
            .collect(),
        finished: false,
    };
    Ok(rankings(&data)
        .into_iter()
        .map(|r| Row {
            rank: r.rank,
            name: r.name,
            total: r.total,
        })
        .collect())
}

#[cfg(test)]
#[path = "flat_test.rs"]
mod tests;
