use crate::{model::*, view};
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    state: State,
    format: String,
    language: String,
    path: Option<String>,
}
fn cell(text: &str) -> String {
    let trimmed = text.trim_start();
    let safe = if trimmed.starts_with(['=', '+', '-', '@']) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}
pub fn save(args: Args, ctx: &dyn TaskContext) -> PluginResult<Value> {
    ctx.stage("validate");
    validate(&args.state, ctx)?;
    let game = args.state.active()?;
    let data = &game.data;
    let zh = args.language.starts_with("zh");
    let labels = if zh {
        [
            "游戏",
            "回合",
            "玩家编号",
            "玩家",
            "本轮得分",
            "累计得分",
            "共同名次",
            "状态",
            "已结算",
            "进行中",
        ]
    } else {
        [
            "Game",
            "Round",
            "Player ID",
            "Player",
            "Round score",
            "Cumulative score",
            "Shared rank",
            "Status",
            "Finished",
            "In progress",
        ]
    };
    ctx.stage("export");
    let content = match args.format.as_str() {
        "json" => serde_json::to_string_pretty(
            &json!({"schemaVersion":1,"game":game,"ranking":view::rankings(data)}),
        )
        .map_err(|_| error("file"))?,
        "csv" => {
            let mut output = format!(
                "\u{feff}{}\r\n",
                labels[..8]
                    .iter()
                    .map(|s| cell(s))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for round in view::rows(data) {
                check(ctx)?;
                let mut prefix = data.clone();
                prefix.rounds.truncate(round.number);
                let ranks = view::rankings(&prefix);
                for (i, player) in data.players.iter().enumerate() {
                    let rank = ranks
                        .iter()
                        .find(|r| r.seat == player.seat)
                        .ok_or_else(|| error("state"))?
                        .rank;
                    output.push_str(&format!(
                        "{},{},{},{},{},{},{},{}\r\n",
                        cell(&data.name),
                        round.number,
                        player.seat,
                        cell(&player.name),
                        round.scores[i],
                        round.cumulative[i],
                        rank,
                        cell(if data.finished { labels[8] } else { labels[9] })
                    ));
                }
            }
            output
        }
        "text" => {
            let mut output = format!(
                "{}\n{} · {} {}\n\n",
                data.name,
                if data.finished { labels[8] } else { labels[9] },
                labels[1],
                data.rounds.len()
            );
            for rank in view::rankings(data) {
                output.push_str(&format!(
                    "{} {} · #{} {} · {} {}\n",
                    labels[6], rank.rank, rank.seat, rank.name, labels[5], rank.total
                ));
            }
            output
        }
        _ => return Err(error("format")),
    };
    check(ctx)?;
    if content.len() > 16 * 1024 * 1024 {
        return Err(error("storage_limit"));
    }
    if let Some(path) = args.path {
        if path.trim().is_empty() {
            return Err(error("file"));
        }
        tf_plugin_guest::net::write_file(&path, content.as_bytes())?;
        Ok(json!({"saved":true}))
    } else if args.format == "text" && content.len() <= 65536 {
        Ok(json!({"content":content}))
    } else {
        Err(error("file"))
    }
}
