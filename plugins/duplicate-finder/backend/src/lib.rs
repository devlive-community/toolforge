//! 重复文件查找插件后端：按大小、开头哈希、完整 BLAKE3 哈希逐级比较，找出内容相同的文件，
//! 并把选中的多余副本移到回收站。

mod find;
mod remove;
mod scan;
#[cfg(test)]
mod test_support;

use std::path::Path;
use std::time::Instant;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct RemoveArgs {
    groups: Vec<remove::Selection>,
}

fn move_to_trash(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // Finder 方式需要自动化授权并播放音效，NSFileManager 方式不需要
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(path).map_err(|e| e.to_string())
    }
}

pub struct DuplicateFinder {
    manifest: Manifest,
}

impl Default for DuplicateFinder {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl DuplicateFinder {
    fn scan(&self, options: scan::Options, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let started = Instant::now();
        ctx.stage("dup.scan");
        let files = scan::scan(&options, |_| {}, || ctx.is_cancelled())?;
        let count = files.len();
        let bytes: u64 = files.iter().map(|f| f.size).sum();
        ctx.log(
            LogLevel::Info,
            "dup.scanned",
            json!({ "files": count, "bytes": bytes }),
        );
        let stage = |code: &str| ctx.stage(code);
        let report = |done: u64, total: u64| ctx.progress(done, total);
        let cancelled = || ctx.is_cancelled();
        let groups = find::find(
            files,
            &find::Progress {
                stage: &stage,
                bytes: &report,
                cancelled: &cancelled,
            },
        )?;
        let duplicates: usize = groups.iter().map(|g| g.files.len() - 1).sum();
        let wasted: u64 = groups.iter().map(|g| g.wasted).sum();
        let elapsed_ms = started.elapsed().as_millis() as u64;
        ctx.log(
            LogLevel::Info,
            "dup.found",
            json!({ "groups": groups.len(), "duplicates": duplicates, "wasted": wasted, "ms": elapsed_ms }),
        );
        Ok(json!({
            "groups": groups,
            "scanned": count,
            "scannedBytes": bytes,
            "duplicates": duplicates,
            "wasted": wasted,
            "elapsedMs": elapsed_ms,
        }))
    }
}

impl ToolPlugin for DuplicateFinder {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, _args: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "scan" => self.scan(parse_args(args)?, ctx),
            "remove" => {
                let args: RemoveArgs = parse_args(args)?;
                ctx.stage("dup.remove");
                let outcome = remove::remove(&args.groups, ctx, &move_to_trash)?;
                ctx.log(
                    LogLevel::Info,
                    "dup.removed",
                    json!({ "count": outcome.removed.len(), "freed": outcome.freed, "skipped": outcome.skipped.len() }),
                );
                to_value(outcome)
            }
            other => Err(unknown_function(other)),
        }
    }
}
