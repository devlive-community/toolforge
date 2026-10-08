//! 文件夹对比插件后端：比较两个文件夹（大小与修改时间，或逐字节内容），列出相同、已变化、
//! 仅在一侧及类型不同的条目；对比变化的文本文件；把选中的条目复制到另一侧。

mod compare;
mod copy;
mod diff;
mod scan;
#[cfg(test)]
mod test_support;

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use compare::Method;
use scan::Ignore;

const MANIFEST: &str = include_str!("../../manifest.json");

fn default_ignore() -> Vec<String> {
    [
        ".git",
        "node_modules",
        ".DS_Store",
        "Thumbs.db",
        "desktop.ini",
    ]
    .map(String::from)
    .to_vec()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompareArgs {
    left: String,
    right: String,
    #[serde(default)]
    method: Method,
    #[serde(default = "default_ignore")]
    ignore: Vec<String>,
    #[serde(default)]
    ignore_hidden: bool,
}

#[derive(Deserialize)]
struct DiffArgs {
    left: String,
    right: String,
    path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Direction {
    ToRight,
    ToLeft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyArgs {
    left: String,
    right: String,
    paths: Vec<String>,
    direction: Direction,
    #[serde(default = "default_ignore")]
    ignore: Vec<String>,
    #[serde(default)]
    ignore_hidden: bool,
}

fn run_compare(args: CompareArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let (left, right) = (scan::root(&args.left)?, scan::root(&args.right)?);
    if left == right {
        return Err(PluginError::new("fc.same_folder"));
    }
    let ignore = Ignore::new(&args.ignore, args.ignore_hidden);
    let stop = || ctx.is_cancelled();
    ctx.stage("fc.scan");
    let (l, r) = rayon::join(
        || scan::scan(&left, &ignore, &stop),
        || scan::scan(&right, &ignore, &stop),
    );
    let (Some(l), Some(r)) = (l, r) else {
        return Err(cancelled());
    };
    ctx.log(
        LogLevel::Info,
        "fc.scanned",
        json!({ "left": l.entries.len(), "right": r.entries.len() }),
    );
    if l.links + r.links > 0 {
        ctx.log(
            LogLevel::Warn,
            "fc.links_skipped",
            json!({ "count": l.links + r.links }),
        );
    }

    let mut same = Default::default();
    if args.method == Method::Content {
        let candidates = compare::candidates(&l.entries, &r.entries);
        let total: u64 = candidates.iter().map(|(_, size)| size).sum();
        ctx.stage("fc.hash");
        ctx.log(
            LogLevel::Info,
            "fc.hashing",
            json!({ "count": candidates.len(), "bytes": total }),
        );
        let done = AtomicU64::new(0);
        same = compare::same_contents(
            &left,
            &right,
            &candidates,
            &|size| {
                let n = done.fetch_add(size, Ordering::Relaxed) + size;
                ctx.progress(n, total);
            },
            &stop,
        );
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
    }
    let (items, summary) = compare::merge(&l.entries, &r.entries, args.method, &same);
    ctx.log(LogLevel::Info, "fc.done", to_value(&summary)?);
    Ok(json!({
        "left": left.to_string_lossy(),
        "right": right.to_string_lossy(),
        "items": items,
        "summary": summary,
    }))
}

fn run_copy(args: CopyArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let (left, right) = (scan::root(&args.left)?, scan::root(&args.right)?);
    let (src, dst) = match args.direction {
        Direction::ToRight => (&left, &right),
        Direction::ToLeft => (&right, &left),
    };
    if args.paths.is_empty() {
        return Err(PluginError::new("fc.nothing_selected"));
    }
    // 选中了文件夹及其中的条目时只复制文件夹
    let mut paths = args.paths.clone();
    paths.sort();
    paths.dedup();
    let paths: Vec<&String> = paths
        .iter()
        .filter(|p| {
            !args
                .paths
                .iter()
                .any(|other| *p != other && p.starts_with(&format!("{other}/")))
        })
        .collect();
    let ignore = Ignore::new(&args.ignore, args.ignore_hidden);
    ctx.stage("fc.copy");
    let (mut files, mut bytes) = (0, 0);
    for (i, rel) in paths.iter().enumerate() {
        let copied = copy::copy(
            src,
            dst,
            rel,
            &ignore,
            &mut |file| ctx.log(LogLevel::Debug, "fc.copied", json!({ "path": file })),
            &|| ctx.is_cancelled(),
        )?;
        files += copied.files;
        bytes += copied.bytes;
        ctx.progress(i as u64 + 1, paths.len() as u64);
    }
    ctx.log(
        LogLevel::Info,
        "fc.copy_done",
        json!({ "files": files, "bytes": bytes }),
    );
    Ok(json!({ "files": files, "bytes": bytes }))
}

pub struct FolderCompare {
    manifest: Manifest,
}

impl Default for FolderCompare {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for FolderCompare {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "diff" => {
                let args: DiffArgs = parse_args(args)?;
                let (left, right) = (scan::root(&args.left)?, scan::root(&args.right)?);
                let output = diff::diff(
                    &copy::resolve(&left, &args.path)?,
                    &copy::resolve(&right, &args.path)?,
                )?;
                to_value(output)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "compare" => run_compare(parse_args(args)?, ctx),
            "copy" => run_copy(parse_args(args)?, ctx),
            _ => self.call(function, args),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
