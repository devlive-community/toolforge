//! LCOV 覆盖率报告查看器（在线安装插件）：离线解析 lcov.info，按目录与文件汇总行、函数、分支覆盖率，
//! 并在源码上标注命中次数与未覆盖区段。不运行测试、不编译项目、不连接 CI。
//!
//! 解析结果缓存在插件实例中（只保留最近一份），后续查看文件详情与导出按 token 取用；
//! 实例重建后 token 失效，界面会重新加载。

mod analysis;
mod lcov;
mod paths;

use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::Value;
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 粘贴文本的上限；更大的报告请打开文件
const MAX_TEXT: usize = 32 * 1024 * 1024;

/// 最近一次解析的报告
static CACHE: Mutex<Option<(u64, Arc<lcov::Report>)>> = Mutex::new(None);

fn store(report: lcov::Report) -> (u64, Arc<lcov::Report>) {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let token = cache.as_ref().map_or(1, |(t, _)| t + 1);
    let report = Arc::new(report);
    *cache = Some((token, report.clone()));
    (token, report)
}

fn cached(token: u64) -> PluginResult<Arc<lcov::Report>> {
    let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match cache.as_ref() {
        Some((t, report)) if *t == token => Ok(report.clone()),
        _ => Err(PluginError::new("lcov.expired")),
    }
}

pub struct LcovViewer {
    manifest: Manifest,
}

impl Default for LcovViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct TextArgs {
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetailArgs {
    token: u64,
    file: String,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Deserialize)]
struct ExportArgs {
    token: u64,
    format: analysis::ExportFormat,
}

fn overview(report: lcov::Report) -> PluginResult<analysis::Overview> {
    if report.files.is_empty() {
        return Err(PluginError::new("lcov.empty"));
    }
    let (token, report) = store(report);
    Ok(analysis::overview(&report, token))
}

/// 逐行读取报告文件，可取消，按字节报告进度
fn load(path: &str, ctx: &dyn TaskContext) -> PluginResult<analysis::Overview> {
    let total = ctx.file_size(path)?;
    let mut reader = BufReader::new(ctx.open_file(path)?);
    let mut parser = lcov::Parser::default();
    let mut line = String::new();
    let mut done = 0u64;
    let mut count = 0usize;
    ctx.stage("lcov.parsing");
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|e| PluginError::new("lcov.read_failed").with("detail", e.to_string()))?;
        if read == 0 {
            break;
        }
        done += read as u64;
        parser.feed(&line);
        count += 1;
        if count.is_multiple_of(20_000) {
            if ctx.is_cancelled() {
                return Err(cancelled());
            }
            ctx.progress(done, total);
        }
    }
    ctx.progress(total, total);
    let report = parser.finish();
    ctx.log(
        LogLevel::Info,
        "lcov.loaded",
        serde_json::json!({ "files": report.files.len(), "records": report.records }),
    );
    overview(report)
}

impl ToolPlugin for LcovViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => {
                let text = parse_args::<TextArgs>(args)?.text;
                if text.len() > MAX_TEXT {
                    return Err(PluginError::new("lcov.too_large").with("mb", MAX_TEXT >> 20));
                }
                to_value(overview(lcov::parse_text(&text))?)
            }
            "detail" => {
                let args: DetailArgs = parse_args(args)?;
                let report = cached(args.token)?;
                let cov = report.files.get(&args.file).ok_or_else(|| {
                    PluginError::new("lcov.file_not_found").with("file", args.file.as_str())
                })?;
                to_value(analysis::detail(&args.file, cov, args.source.as_deref()))
            }
            "resolve" => to_value(paths::resolve(&parse_args(args)?)?),
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                to_value(analysis::export(&*cached(args.token)?, args.format)?)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "load" => to_value(load(&parse_args::<PathArgs>(args)?.path, ctx)?),
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(LcovViewer);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
