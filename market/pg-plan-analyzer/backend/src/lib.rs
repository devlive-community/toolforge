//! PostgreSQL 执行计划分析（在线安装插件）：离线解析 EXPLAIN (FORMAT JSON)，
//! 构建计划树、按 loops 口径拆分耗时并提示估计偏差与排序落盘。
//! 不连接数据库、不代替用户运行查询。

mod analysis;
mod plan;

use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use analysis::Report;

const MANIFEST: &str = include_str!("../../manifest.json");
const READ_CHUNK: u64 = 1024 * 1024;

/// 最近一次分析结果（导出用）
static LAST: Mutex<Option<(u64, Arc<Report>)>> = Mutex::new(None);

#[derive(Deserialize)]
struct TextArgs {
    text: String,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    handle: u64,
    format: Format,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    Markdown,
    Json,
}

fn store(mut report: Report) -> (u64, Arc<Report>) {
    let mut cache = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let handle = cache.as_ref().map_or(1, |(t, _)| t + 1);
    report.handle = handle;
    let report = Arc::new(report);
    *cache = Some((handle, report.clone()));
    (handle, report)
}

fn cached(handle: u64) -> PluginResult<Arc<Report>> {
    let cache = LAST.lock().unwrap_or_else(|e| e.into_inner());
    match cache.as_ref() {
        Some((t, report)) if *t == handle => Ok(report.clone()),
        _ => Err(PluginError::new("pg.no_result")),
    }
}

fn load(path: &str, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let total = ctx.file_size(path)?;
    if total > plan::MAX_FILE {
        return Err(PluginError::new("pg.file_too_large").with("mb", plan::MAX_FILE >> 20));
    }
    let mut file = ctx.open_file(path)?;
    ctx.stage("pg.reading");
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|e| PluginError::new("pg.read_failed").with("detail", e.to_string()))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        done += read as u64;
        if done / READ_CHUNK != (done - read as u64) / READ_CHUNK {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            ctx.progress(done, total);
        }
    }
    ctx.progress(total, total);
    ctx.stage("pg.parsing");
    let report = analysis::analyze_slice(&bytes, &plan::Limits::default())?;
    let (_, report) = store(report);
    ctx.log(
        LogLevel::Info,
        "pg.loaded",
        json!({ "statements": report.statements.len(), "nodes": report.node_count() }),
    );
    to_value(&*report)
}

pub struct PgPlanAnalyzer {
    manifest: Manifest,
}

impl Default for PgPlanAnalyzer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PgPlanAnalyzer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => {
                let text = parse_args::<TextArgs>(args)?.text;
                if text.len() > plan::MAX_TEXT {
                    return Err(PluginError::new("pg.too_large").with("mb", plan::MAX_TEXT >> 20));
                }
                let report = analysis::analyze_text(&text, &plan::Limits::default())?;
                let (_, report) = store(report);
                to_value(&*report)
            }
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let report = cached(args.handle)?;
                match args.format {
                    Format::Markdown => to_value(json!({
                        "content": analysis::markdown(&report),
                        "ext": "md",
                    })),
                    Format::Json => to_value(json!({
                        "content": serde_json::to_string_pretty(&*report).unwrap_or_default(),
                        "ext": "json",
                    })),
                }
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "load" => load(&parse_args::<PathArgs>(args)?.path, ctx),
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(PgPlanAnalyzer);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
