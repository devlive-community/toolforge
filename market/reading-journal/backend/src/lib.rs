//! 书影手账后端：作品与经历分开保存，进度与完成分别确认，筛选、年度回顾、导出与恢复。
//! 手账由前端保存在 ToolForge SQLite 的插件状态里（不新建数据库），这里只做无状态计算。

mod backup;
mod date;
mod model;
mod ops;
mod photo;
mod view;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 后端分发的全部函数；宿主只放行 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 10] = [
    "empty",
    "view",
    "history",
    "review",
    "apply",
    "cover",
    "exportList",
    "exportBackup",
    "restore",
    "recap",
];

pub struct ReadingJournal {
    manifest: Manifest,
}

impl Default for ReadingJournal {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryArgs {
    journal: model::Journal,
    work_id: String,
}

#[derive(Debug, Deserialize)]
struct ReviewArgs {
    journal: model::Journal,
    year: i32,
}

/// 命令行 / Agent：读取导出的备份文件，给出某一年的年度回顾
#[derive(Debug, Deserialize)]
struct RecapArgs {
    path: String,
    #[serde(default)]
    year: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct CoverArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct ListArgs {
    journal: model::Journal,
    headers: Vec<String>,
    #[serde(default)]
    labels: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct BackupArgs {
    journal: model::Journal,
    #[serde(default)]
    covers: BTreeMap<String, String>,
    today: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreArgs {
    journal: model::Journal,
    text: String,
    mode: backup::Mode,
    revision: u64,
    #[serde(default)]
    preview: bool,
}

fn checked_path(path: &str) -> PluginResult<()> {
    if path.is_empty() || path.len() > 4096 {
        return Err(model::error("bad_path"));
    }
    Ok(())
}

fn recap(args: RecapArgs) -> PluginResult<Value> {
    checked_path(&args.path)?;
    let bytes = tf_plugin_guest::net::read_to_end(&args.path, backup::MAX_BACKUP as u64)?;
    let text = String::from_utf8(bytes).map_err(|_| model::error("bad_backup"))?;
    let (journal, _) = backup::parse_backup(&text)?;
    let year = args
        .year
        .unwrap_or_else(|| jiff::Zoned::now().year().into());
    to_value(view::review(&journal, year)?)
}

impl ToolPlugin for ReadingJournal {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "empty" => to_value(model::Journal {
                schema: model::SCHEMA,
                ..model::Journal::default()
            }),
            "view" => to_value(view::view(&parse_args(args)?)?),
            "history" => {
                let args: HistoryArgs = parse_args(args)?;
                let (work, sessions) = view::history(&args.journal, &args.work_id)?;
                Ok(json!({ "work": work, "sessions": sessions }))
            }
            "review" => {
                let args: ReviewArgs = parse_args(args)?;
                to_value(view::review(&args.journal, args.year)?)
            }
            "apply" => to_value(ops::apply(parse_args(args)?)?),
            "cover" => {
                let args: CoverArgs = parse_args(args)?;
                checked_path(&args.path)?;
                let bytes = tf_plugin_guest::net::read_to_end(&args.path, photo::MAX_SOURCE)?;
                Ok(json!({ "data": photo::compress(&bytes)? }))
            }
            "exportList" => {
                let args: ListArgs = parse_args(args)?;
                Ok(
                    json!({ "content": backup::list_csv(&args.journal, &args.headers, &args.labels)? }),
                )
            }
            "exportBackup" => {
                let args: BackupArgs = parse_args(args)?;
                Ok(
                    json!({ "content": backup::backup_json(&args.journal, &args.covers, &args.today)? }),
                )
            }
            "restore" => {
                let args: RestoreArgs = parse_args(args)?;
                to_value(backup::restore(
                    &args.journal,
                    &args.text,
                    args.mode,
                    args.revision,
                    args.preview,
                )?)
            }
            "recap" => recap(parse_args(args)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(ReadingJournal);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
