//! 闪卡与间隔复习后端：调度（固定档位）、队列构建、CSV 导入、备份恢复。
//! 卡片与评分历史由前端持久化（usePluginState → ToolForge SQLite），
//! 这里只做无状态计算与校验；日期口径为前端传入的本地日历日。

mod backup;
mod csv_import;
mod id;
mod schedule;

use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct Flashcards {
    manifest: Manifest,
}

impl Default for Flashcards {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for Flashcards {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "review" => to_value(schedule::review(parse_args(args)?)?),
            "queue" => to_value(schedule::queue(parse_args(args)?)?),
            "import_csv" => to_value(csv_import::import_csv(parse_args(args)?)?),
            "backup" => to_value(backup::export(parse_args(args)?)?),
            "restore_preview" => to_value(backup::restore_preview(parse_args(args)?)?),
            "restore_apply" => to_value(backup::restore_apply(parse_args(args)?)?),
            "new_id" => to_value(id::new_id()),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Flashcards);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
