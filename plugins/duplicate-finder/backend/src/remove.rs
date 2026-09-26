//! 移除多余副本：删除前重新计算哈希，确认要删的文件与保留的文件内容仍然一致，
//! 每组至少保留一个文件。文件移到系统回收站（废纸篓），可以恢复。

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, cancelled};

use crate::find::hash_file;

#[derive(Debug, Clone, Deserialize)]
pub struct Selection {
    pub hash: String,
    pub keep: Vec<String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub removed: Vec<String>,
    /// 内容已变化、找不到或删除失败而跳过的文件
    pub skipped: Vec<String>,
    pub freed: u64,
}

pub fn remove(
    selections: &[Selection],
    ctx: &dyn TaskContext,
    trash: &dyn Fn(&Path) -> Result<(), String>,
) -> PluginResult<Outcome> {
    if selections.iter().any(|s| s.keep.is_empty()) {
        return Err(PluginError::new("dup.keep_one"));
    }
    let total: usize = selections.iter().map(|s| s.remove.len()).sum();
    let mut outcome = Outcome::default();
    let is_cancelled = || ctx.is_cancelled();
    let mut done = 0;
    for selection in selections {
        // 至少有一个保留的文件仍然存在且内容不变
        let kept = selection.keep.iter().any(|path| {
            hash_file(Path::new(path), None, &is_cancelled)
                .ok()
                .flatten()
                .is_some_and(|h| h == selection.hash)
        });
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        for path in &selection.remove {
            done += 1;
            ctx.progress(done as u64, total as u64);
            let target = Path::new(path);
            let size = std::fs::metadata(target).map(|m| m.len()).unwrap_or(0);
            let same = kept
                && hash_file(target, None, &is_cancelled)
                    .ok()
                    .flatten()
                    .is_some_and(|h| h == selection.hash);
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            if !same {
                ctx.log(LogLevel::Warn, "dup.changed", json!({ "path": path }));
                outcome.skipped.push(path.clone());
                continue;
            }
            match trash(target) {
                Ok(()) => {
                    ctx.log(
                        LogLevel::Info,
                        "dup.trashed",
                        json!({ "path": path, "size": size }),
                    );
                    outcome.removed.push(path.clone());
                    outcome.freed += size;
                }
                Err(detail) => {
                    ctx.log(
                        LogLevel::Error,
                        "dup.trash_failed",
                        json!({ "path": path, "detail": detail }),
                    );
                    outcome.skipped.push(path.clone());
                }
            }
        }
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "remove_test.rs"]
mod tests;
