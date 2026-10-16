//! Source Map 堆栈还原（在线安装插件）：离线解析 v3 Source Map（含内嵌
//! sections 的 index map），把压缩产物的报错位置批量还原为源文件、行列与
//! 符号名。不请求 URL、不执行 JavaScript；源码只来自内嵌 sourcesContent，
//! 产物与 map 的关联由用户显式给出。

mod sourcemap;
mod stack;
mod symbolicate;
mod vlq;

use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use sourcemap::SourceMapData;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 粘贴堆栈文本的上限
const MAX_STACK: usize = 512 * 1024;
/// 一次还原的行数上限
const MAX_FRAMES: usize = 2000;
/// 读取文件时每读取多大报告一次进度
const READ_CHUNK: u64 = 1024 * 1024;

/// 最近一次解析的 map（只保留一份，句柄递增使旧引用失效）
static MAPS: Mutex<Option<(u64, Arc<SourceMapData>)>> = Mutex::new(None);
/// 最近一次还原结果（导出用）
static LAST: Mutex<Option<(u64, Arc<symbolicate::Symbolication>)>> = Mutex::new(None);

fn store_map(data: SourceMapData) -> (u64, Arc<SourceMapData>) {
    let mut cache = MAPS.lock().unwrap_or_else(|e| e.into_inner());
    let handle = cache.as_ref().map_or(1, |(t, _)| t + 1);
    let data = Arc::new(data);
    *cache = Some((handle, data.clone()));
    (handle, data)
}

fn cached_map(handle: u64) -> PluginResult<Arc<SourceMapData>> {
    let cache = MAPS.lock().unwrap_or_else(|e| e.into_inner());
    match cache.as_ref() {
        Some((t, data)) if *t == handle => Ok(data.clone()),
        _ => Err(PluginError::new("sourcemap.expired")),
    }
}

fn store_result(
    result: symbolicate::Symbolication,
    handle: u64,
) -> Arc<symbolicate::Symbolication> {
    let mut cache = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let result = Arc::new(result);
    *cache = Some((handle, result.clone()));
    result
}

fn cached_result(handle: u64) -> PluginResult<Arc<symbolicate::Symbolication>> {
    let cache = LAST.lock().unwrap_or_else(|e| e.into_inner());
    match cache.as_ref() {
        Some((t, result)) if *t == handle => Ok(result.clone()),
        _ => Err(PluginError::new("sourcemap.no_result")),
    }
}

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
struct SymbolicateArgs {
    handle: u64,
    #[serde(default)]
    stack: Option<String>,
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    position: Option<PositionArgs>,
}

#[derive(Deserialize)]
struct PositionArgs {
    line: u64,
    column: u64,
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

fn symbolicate(args: SymbolicateArgs) -> PluginResult<Value> {
    if let Some(stack) = args.stack.as_deref()
        && stack.len() > MAX_STACK
    {
        return Err(PluginError::new("sourcemap.stack_too_large").with("kb", MAX_STACK >> 10));
    }
    let map = cached_map(args.handle)?;
    let position = args.position.map(|p| (p.line, p.column));
    let result = symbolicate::run(&map, args.stack.as_deref(), args.asset.as_deref(), position)?;
    let result = store_result(result, args.handle);
    to_value(&*result)
}

/// 读取 map 文件：分块读入并报告进度，可取消；解析在读取完成后进行
fn load(path: &str, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let total = ctx.file_size(path)?;
    if total > sourcemap::MAX_FILE {
        return Err(
            PluginError::new("sourcemap.file_too_large").with("mb", sourcemap::MAX_FILE >> 20)
        );
    }
    let mut file = ctx.open_file(path)?;
    ctx.stage("sourcemap.reading");
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|e| PluginError::new("sourcemap.read_failed").with("detail", e.to_string()))?;
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
    ctx.stage("sourcemap.parsing");
    let data = sourcemap::parse_slice(&bytes)?;
    let (token, data) = store_map(data);
    ctx.log(
        LogLevel::Info,
        "sourcemap.loaded",
        json!({ "segments": data.segments, "sources": data.sources_len() }),
    );
    to_value(sourcemap::overview(&data, token))
}

pub struct SourceMapSymbolicator {
    manifest: Manifest,
}

impl Default for SourceMapSymbolicator {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for SourceMapSymbolicator {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => {
                let text = parse_args::<TextArgs>(args)?.text;
                if text.len() > sourcemap::MAX_TEXT {
                    return Err(PluginError::new("sourcemap.too_large")
                        .with("mb", sourcemap::MAX_TEXT >> 20));
                }
                let data = sourcemap::parse_text(&text)?;
                let (token, data) = store_map(data);
                to_value(sourcemap::overview(&data, token))
            }
            "symbolicate" => symbolicate(parse_args(args)?),
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let result = cached_result(args.handle)?;
                match args.format {
                    Format::Markdown => to_value(json!({
                        "content": symbolicate::markdown(&result),
                        "ext": "md",
                    })),
                    Format::Json => to_value(json!({
                        "content": serde_json::to_string_pretty(&*result)
                            .unwrap_or_default(),
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

tf_plugin_guest::export_plugin!(SourceMapSymbolicator);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
