//! 收藏品与借还台账后端：库存守恒、借还事务、撤销、视图汇总、导出与恢复。
//! 台账由前端保存在 ToolForge SQLite 的插件状态里（不新建数据库），
//! 这里只做无状态计算：每次调用传入完整台账，返回校验后的新台账或视图。

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
const FUNCTIONS: [&str; 8] = [
    "empty",
    "view",
    "history",
    "apply",
    "photo",
    "exportList",
    "exportBackup",
    "restore",
];

pub struct CollectionLedger {
    manifest: Manifest,
}

impl Default for CollectionLedger {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryArgs {
    ledger: model::Ledger,
    item_id: String,
    today: String,
}

#[derive(Debug, Deserialize)]
struct PhotoArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct ListArgs {
    ledger: model::Ledger,
    headers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BackupArgs {
    ledger: model::Ledger,
    #[serde(default)]
    photos: BTreeMap<String, String>,
    today: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreArgs {
    ledger: model::Ledger,
    text: String,
    mode: backup::Mode,
    revision: u64,
    #[serde(default)]
    preview: bool,
}

fn photo_call(args: Value) -> PluginResult<Value> {
    let args: PhotoArgs = parse_args(args)?;
    if args.path.is_empty() || args.path.len() > 4096 {
        return Err(model::error("bad_photo"));
    }
    let bytes = tf_plugin_guest::net::read_to_end(&args.path, photo::MAX_SOURCE)?;
    Ok(json!({ "data": photo::compress(&bytes)? }))
}

impl ToolPlugin for CollectionLedger {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "empty" => to_value(model::Ledger {
                schema: model::SCHEMA,
                ..model::Ledger::default()
            }),
            "view" => to_value(view::view(&parse_args(args)?)?),
            "history" => {
                let args: HistoryArgs = parse_args(args)?;
                let (item, loans) = view::history(&args.ledger, &args.item_id, &args.today)?;
                Ok(json!({ "item": item, "loans": loans }))
            }
            "apply" => to_value(ops::apply(parse_args(args)?)?),
            "photo" => photo_call(args),
            "exportList" => {
                let args: ListArgs = parse_args(args)?;
                Ok(json!({ "content": backup::list_csv(&args.ledger, &args.headers)? }))
            }
            "exportBackup" => {
                let args: BackupArgs = parse_args(args)?;
                Ok(
                    json!({ "content": backup::backup_json(&args.ledger, &args.photos, &args.today)? }),
                )
            }
            "restore" => {
                let args: RestoreArgs = parse_args(args)?;
                to_value(backup::restore(
                    &args.ledger,
                    &args.text,
                    args.mode,
                    args.revision,
                    args.preview,
                )?)
            }
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(CollectionLedger);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
