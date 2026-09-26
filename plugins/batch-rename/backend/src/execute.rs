//! 执行重命名：先把所有源文件改成临时名，再改成目标名，因此互换（a↔b）、
//! 循环改名以及只改大小写都能正确完成。任何一步失败都会回滚已完成的步骤。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};

use crate::plan::path_key;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Move {
    pub from: String,
    pub to: String,
}

fn rename(from: &Path, to: &Path) -> PluginResult<()> {
    std::fs::rename(from, to).map_err(|e| {
        PluginError::new("rename.failed")
            .with("path", from.to_string_lossy().as_ref())
            .with("detail", e.to_string())
    })
}

fn temporary(from: &Path, index: usize) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros());
    from.with_file_name(format!(
        ".tf-rename-{}-{stamp}-{index}.tmp",
        std::process::id()
    ))
}

/// 尽力回滚：按相反顺序把已移动的文件移回原处
fn rollback(done: &[(PathBuf, PathBuf)]) {
    for (from, to) in done.iter().rev() {
        let _ = std::fs::rename(to, from);
    }
}

pub fn execute(moves: &[Move], ctx: &dyn TaskContext) -> PluginResult<Vec<Move>> {
    let total = moves.len() as u64 * 2;
    // 第一阶段：源文件 → 临时名
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(moves.len());
    for (index, step) in moves.iter().enumerate() {
        let from = PathBuf::from(&step.from);
        let temp = temporary(&from, index);
        if let Err(err) = rename(&from, &temp) {
            rollback(&staged);
            return Err(err);
        }
        staged.push((from, temp));
        ctx.progress(index as u64 + 1, total);
    }
    // 第二阶段：临时名 → 目标名；目标此时必须不存在
    let mut finished: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(moves.len());
    for (index, (step, (_, temp))) in moves.iter().zip(&staged).enumerate() {
        let to = PathBuf::from(&step.to);
        let result = if std::fs::symlink_metadata(&to).is_ok() {
            Err(PluginError::new("rename.target_appeared").with("path", step.to.as_str()))
        } else {
            rename(temp, &to)
        };
        if let Err(err) = result {
            rollback(
                &finished
                    .iter()
                    .map(|(t, to)| (t.clone(), to.clone()))
                    .collect::<Vec<_>>(),
            );
            rollback(&staged);
            ctx.log(
                LogLevel::Error,
                "rename.rolled_back",
                json!({ "code": err.code }),
            );
            return Err(err);
        }
        finished.push((temp.clone(), to));
        ctx.log(
            LogLevel::Info,
            "rename.renamed",
            json!({
                "from": Path::new(&step.from).file_name().map(|n| n.to_string_lossy().into_owned()),
                "to": Path::new(&step.to).file_name().map(|n| n.to_string_lossy().into_owned()),
            }),
        );
        ctx.progress(moves.len() as u64 + index as u64 + 1, total);
    }
    Ok(moves.to_vec())
}

/// 撤销：把上次的结果改回原名；目标已被移走或原名已被占用时拒绝
pub fn undo(journal: &[Move], ctx: &dyn TaskContext) -> PluginResult<Vec<Move>> {
    let reverse: Vec<Move> = journal
        .iter()
        .map(|m| Move {
            from: m.to.clone(),
            to: m.from.clone(),
        })
        .collect();
    for step in &reverse {
        if std::fs::symlink_metadata(&step.from).is_err() {
            return Err(PluginError::new("rename.undo_missing").with("path", step.from.as_str()));
        }
        let case_only = path_key(Path::new(&step.from)) == path_key(Path::new(&step.to));
        let vacated = reverse
            .iter()
            .any(|other| path_key(Path::new(&other.from)) == path_key(Path::new(&step.to)));
        if !case_only && !vacated && std::fs::symlink_metadata(&step.to).is_ok() {
            return Err(PluginError::new("rename.undo_occupied").with("path", step.to.as_str()));
        }
    }
    execute(&reverse, ctx)
}

#[cfg(test)]
#[path = "execute_test.rs"]
mod tests;
