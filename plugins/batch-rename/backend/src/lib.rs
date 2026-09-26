//! 批量重命名插件后端：按规则生成新文件名并预览冲突，两阶段安全改名，支持撤销上一次操作。

mod execute;
mod plan;
mod rules;

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use execute::Move;
use plan::{ExifCache, PlanArgs, Status};

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_FILES: usize = 10_000;

#[derive(Deserialize)]
struct ScanArgs {
    paths: Vec<String>,
    #[serde(default)]
    recursive: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Scanned {
    paths: Vec<String>,
    /// 超过上限被忽略的文件数
    skipped: usize,
}

#[derive(Deserialize)]
struct UndoArgs {
    journal: Vec<Move>,
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

fn walk(path: &Path, recursive: bool, top: bool, out: &mut Vec<String>, skipped: &mut usize) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.is_dir() {
        if !top && !recursive {
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| !is_hidden(p))
            .collect();
        children.sort_by(|a, b| plan::natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        for child in children {
            walk(&child, recursive, false, out, skipped);
        }
    } else if out.len() < MAX_FILES {
        out.push(path.to_string_lossy().into_owned());
    } else {
        *skipped += 1;
    }
}

/// 展开文件与文件夹：文件夹取其中的文件，recursive 时包含子文件夹；跳过隐藏文件
fn scan(args: &ScanArgs) -> Scanned {
    let mut paths = Vec::new();
    let mut skipped = 0;
    for path in &args.paths {
        walk(
            Path::new(path),
            args.recursive,
            true,
            &mut paths,
            &mut skipped,
        );
    }
    Scanned { paths, skipped }
}

pub struct BatchRename {
    manifest: Manifest,
    exif: ExifCache,
}

impl Default for BatchRename {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            exif: ExifCache::default(),
        }
    }
}

impl BatchRename {
    fn apply(&self, args: PlanArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let started = Instant::now();
        ctx.stage("rename.check");
        let plan = plan::build(&args, &self.exif)?;
        if plan.problems > 0 {
            return Err(PluginError::new("rename.has_conflicts").with("count", plan.problems));
        }
        let moves: Vec<Move> = plan
            .items
            .iter()
            .filter(|i| i.status == Status::Ok)
            .map(|i| Move {
                from: i.path.clone(),
                to: Path::new(&i.path)
                    .with_file_name(&i.to)
                    .to_string_lossy()
                    .into_owned(),
            })
            .collect();
        if moves.is_empty() {
            return Err(PluginError::new("rename.nothing_to_do"));
        }
        ctx.stage("rename.apply");
        let journal = execute::execute(&moves, ctx)?;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        ctx.log(
            LogLevel::Info,
            "rename.done",
            json!({ "count": journal.len(), "ms": elapsed_ms }),
        );
        Ok(json!({ "journal": journal, "elapsedMs": elapsed_ms }))
    }
}

impl ToolPlugin for BatchRename {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "scan" => to_value(scan(&parse_args(args)?)),
            "preview" => to_value(plan::build(&parse_args(args)?, &self.exif)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "apply" => self.apply(parse_args(args)?, ctx),
            "undo" => {
                let args: UndoArgs = parse_args(args)?;
                ctx.stage("rename.undo");
                let journal = execute::undo(&args.journal, ctx)?;
                ctx.log(
                    LogLevel::Info,
                    "rename.undone",
                    json!({ "count": journal.len() }),
                );
                Ok(json!({ "journal": journal }))
            }
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
