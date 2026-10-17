//! WebAssembly 模块检查器（在线安装插件）：只读分析 core module 的 section 体积、导入导出、
//! 函数、内存与表限制、custom section，用标准校验器检查模块，并可查看 / 导出 WAT。
//! 不实例化或执行模块，不调用导入，不模拟 WASI；component model 暂不支持。
//!
//! 最近一次打开的模块缓存在插件实例中，后续查询按 token 取用；实例重建后 token 失效，界面会提示重新打开。

#[cfg(test)]
mod fixtures;
mod model;
mod validate;
mod wat;

use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{
    LogLevel, Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, cancelled, parse_args,
    to_value, unknown_function,
};

use model::{ExportRow, FunctionRow, ImportRow, Module, Problem};
use validate::Validation;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 文件大小上限
const MAX_FILE: u64 = 128 * 1024 * 1024;
/// WAT 文本上限，超出后截断
const MAX_WAT: usize = 64 * 1024 * 1024;
/// 单页最多返回的条目
const MAX_PAGE: usize = 500;
/// 概览中列出的最大函数个数
const LARGEST: usize = 10;
/// 读取文件的块大小
const CHUNK: usize = 1 << 20;

struct Loaded {
    token: u64,
    name: String,
    bytes: Vec<u8>,
    module: Module,
    validation: Validation,
    wat: Mutex<Option<Arc<wat::Wat>>>,
}

/// 最近一次打开的模块
static CACHE: Mutex<Option<Arc<Loaded>>> = Mutex::new(None);

fn next_token() -> u64 {
    CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map_or(1, |l| l.token + 1)
}

fn cached(token: u64) -> PluginResult<Arc<Loaded>> {
    match CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(loaded) if loaded.token == token => Ok(loaded.clone()),
        _ => Err(PluginError::new("wasm.expired")),
    }
}

fn problem_error(p: Problem) -> PluginError {
    let mut err = PluginError::new(p.code).with("detail", p.message);
    if let Some(offset) = p.offset {
        err = err.with("offset", offset);
    }
    err
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview<'a> {
    token: u64,
    name: &'a str,
    #[serde(flatten)]
    module: &'a Module,
    validation: &'a Validation,
    imports: usize,
    exports: usize,
    largest: Vec<&'a FunctionRow>,
}

fn overview(loaded: &Loaded) -> Overview<'_> {
    let mut largest: Vec<&FunctionRow> = loaded.module.functions.iter().collect();
    largest.sort_by(|a, b| b.size.cmp(&a.size).then(a.index.cmp(&b.index)));
    largest.truncate(LARGEST);
    Overview {
        token: loaded.token,
        name: &loaded.name,
        module: &loaded.module,
        validation: &loaded.validation,
        imports: loaded.module.imports.len(),
        exports: loaded.module.exports.len(),
        largest,
    }
}

/// 解析并校验字节，放入缓存
fn inspect(name: &str, bytes: Vec<u8>, ctx: &dyn TaskContext) -> PluginResult<Arc<Loaded>> {
    let stop = || ctx.is_cancelled();
    ctx.stage("wasm.parsing");
    let module = match model::analyze(&bytes, &stop) {
        Ok(module) => module,
        Err(p) if p.code == "task.cancelled" => return Err(cancelled()),
        Err(p) => return Err(problem_error(p)),
    };
    ctx.stage("wasm.validating");
    let validation = validate::validate(&bytes, &stop).ok_or_else(cancelled)?;
    ctx.log(
        LogLevel::Info,
        "wasm.loaded",
        json!({
            "sections": module.sections.len(),
            "functions": module.functions.len(),
            "status": validation.status,
        }),
    );
    let loaded = Arc::new(Loaded {
        token: next_token(),
        name: name.to_owned(),
        bytes,
        module,
        validation,
        wat: Mutex::new(None),
    });
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some(loaded.clone());
    Ok(loaded)
}

/// 分块读取文件，可取消，按字节报告进度
fn read(path: &str, ctx: &dyn TaskContext) -> PluginResult<Vec<u8>> {
    let total = ctx.file_size(path)?;
    if total > MAX_FILE {
        return Err(PluginError::new("wasm.too_large").with("mb", MAX_FILE >> 20));
    }
    ctx.stage("wasm.reading");
    let mut reader = ctx.open_file(path)?;
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| PluginError::new("wasm.read_failed").with("detail", e.to_string()))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() as u64 > MAX_FILE {
            return Err(PluginError::new("wasm.too_large").with("mb", MAX_FILE >> 20));
        }
        if ctx.is_cancelled() {
            return Err(cancelled());
        }
        ctx.progress(bytes.len() as u64, total.max(bytes.len() as u64));
    }
    Ok(bytes)
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct TokenArgs {
    token: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListArgs {
    token: u64,
    kind: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    query: String,
    /// functions：index（默认）或 size
    #[serde(default)]
    sort: Option<String>,
}

fn default_limit() -> usize {
    50
}

#[derive(Deserialize)]
struct FunctionArgs {
    token: u64,
    index: u32,
}

#[derive(Deserialize)]
struct ExportArgs {
    token: u64,
    format: String,
}

#[derive(Serialize)]
struct Page<T> {
    total: usize,
    items: Vec<T>,
}

fn page<T: Clone>(items: Vec<&T>, offset: usize, limit: usize) -> Page<T> {
    let limit = limit.clamp(1, MAX_PAGE);
    Page {
        total: items.len(),
        items: items
            .into_iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect(),
    }
}

fn matches(query: &str, fields: &[&str]) -> bool {
    query.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(query))
}

fn list(loaded: &Loaded, args: ListArgs) -> PluginResult<Value> {
    let q = args.query.trim().to_lowercase();
    let m = &loaded.module;
    match args.kind.as_str() {
        "imports" => {
            let items: Vec<&ImportRow> = m
                .imports
                .iter()
                .filter(|i| matches(&q, &[&i.module, &i.name, i.kind, &i.desc]))
                .collect();
            to_value(page(items, args.offset, args.limit))
        }
        "exports" => {
            let items: Vec<&ExportRow> = m
                .exports
                .iter()
                .filter(|e| matches(&q, &[&e.name, e.kind, &e.desc]))
                .collect();
            to_value(page(items, args.offset, args.limit))
        }
        "functions" => {
            let mut items: Vec<&FunctionRow> = m
                .functions
                .iter()
                .filter(|f| {
                    let index = f.index.to_string();
                    matches(
                        &q,
                        &[
                            &index,
                            f.name.as_deref().unwrap_or(""),
                            f.export.as_deref().unwrap_or(""),
                            &f.signature,
                        ],
                    )
                })
                .collect();
            if args.sort.as_deref() == Some("size") {
                items.sort_by(|a, b| b.size.cmp(&a.size).then(a.index.cmp(&b.index)));
            }
            to_value(page(items, args.offset, args.limit))
        }
        other => Err(PluginError::new("wasm.invalid_kind").with("kind", other)),
    }
}

fn wat_of(loaded: &Loaded) -> PluginResult<Arc<wat::Wat>> {
    loaded
        .wat
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| PluginError::new("wasm.wat_not_ready"))
}

/// 生成（或取出已生成的）WAT
fn print_wat(loaded: &Loaded, ctx: &dyn TaskContext) -> PluginResult<Value> {
    let existing = loaded.wat.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let wat = match existing {
        Some(wat) => wat,
        None => {
            ctx.stage("wasm.printing");
            let printed = wat::print(&loaded.bytes, MAX_WAT, &|| ctx.is_cancelled());
            let wat = match printed {
                wat::Printed::Done(wat) => Arc::new(wat),
                wat::Printed::Cancelled => return Err(cancelled()),
                wat::Printed::Failed(p) => return Err(problem_error(p)),
            };
            *loaded.wat.lock().unwrap_or_else(|e| e.into_inner()) = Some(wat.clone());
            wat
        }
    };
    let (outline, outline_truncated) = wat.outline();
    Ok(json!({
        "bytes": wat.text.len(),
        "truncated": wat.truncated,
        "functions": wat.functions.len(),
        "outline": outline,
        "outlineTruncated": outline_truncated,
    }))
}

fn export(loaded: &Loaded, format: &str) -> PluginResult<Value> {
    match format {
        "json" => {
            let m = &loaded.module;
            let report = json!({
                "file": loaded.name,
                "summary": overview(loaded),
                "imports": m.imports,
                "exports": m.exports,
                "functions": m.functions,
            });
            let content = serde_json::to_string_pretty(&report).map_err(|e| {
                PluginError::new("plugin.serialize_failed").with("detail", e.to_string())
            })?;
            Ok(json!({ "content": content, "ext": "json" }))
        }
        "wat" => {
            let wat = wat_of(loaded)?;
            if wat.truncated {
                return Err(PluginError::new("wasm.wat_truncated").with("mb", MAX_WAT >> 20));
            }
            Ok(json!({ "content": wat.text, "ext": "wat" }))
        }
        other => Err(PluginError::new("wasm.invalid_format").with("format", other)),
    }
}

pub struct WasmInspector {
    manifest: Manifest,
}

impl Default for WasmInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for WasmInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "list" => {
                let args: ListArgs = parse_args(args)?;
                list(&*cached(args.token)?, args)
            }
            "function_wat" => {
                let args: FunctionArgs = parse_args(args)?;
                let loaded = cached(args.token)?;
                let wat = wat_of(&loaded)?;
                let not_found =
                    || PluginError::new("wasm.function_not_found").with("index", args.index);
                let ordinal = args
                    .index
                    .checked_sub(loaded.module.counts.imported_functions)
                    .ok_or_else(not_found)?;
                let text = wat.function(ordinal as usize).ok_or_else(|| {
                    if wat.truncated {
                        PluginError::new("wasm.wat_truncated").with("mb", MAX_WAT >> 20)
                    } else {
                        not_found()
                    }
                })?;
                Ok(json!({ "text": text }))
            }
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                export(&*cached(args.token)?, &args.format)
            }
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "load" => {
                let path = parse_args::<PathArgs>(args)?.path;
                let bytes = read(&path, ctx)?;
                let loaded = inspect(file_name(&path), bytes, ctx)?;
                to_value(overview(&loaded))
            }
            "wat" => {
                let token = parse_args::<TokenArgs>(args)?.token;
                print_wat(&*cached(token)?, ctx)
            }
            _ => self.call(function, args),
        }
    }
}

tf_plugin_guest::export_plugin!(WasmInspector);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
