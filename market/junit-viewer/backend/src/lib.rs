//! Offline JUnit inspection. Full logs remain in Rust and are read in chunks.
mod export;
mod model;
mod parser;
mod view;

use model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::{Arc, Mutex},
};
use tf_plugin_api::{
    LogLevel, Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

#[derive(Default)]
struct Cache {
    next: u64,
    current: Option<(String, Arc<Report>)>,
}
pub struct JunitViewer {
    manifest: Manifest,
    cache: Mutex<Cache>,
}
impl Default for JunitViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
            cache: Mutex::default(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadArgs {
    paths: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetailArgs {
    handle: String,
    id: usize,
    kind: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "page_size")]
    limit: usize,
}
fn page_size() -> usize {
    PAGE
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContentArgs {
    handle: String,
    id: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default = "chunk_size")]
    limit: usize,
}
fn chunk_size() -> usize {
    8192
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportArgs {
    handle: String,
    format: String,
    path: String,
    #[serde(default)]
    filter: view::Filter,
}
impl JunitViewer {
    fn report(&self, handle: &str) -> PluginResult<Arc<Report>> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .current
            .as_ref()
            .filter(|(id, _)| id == handle)
            .map(|(_, r)| r.clone())
            .ok_or_else(|| error("expired"))
    }
    fn load(&self, args: LoadArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if args.paths.is_empty()
            || args.paths.len() > 32
            || args.paths.iter().any(|p| p.is_empty() || p.len() > 4096)
        {
            return Err(error("files"));
        }
        let mut report = Report::default();
        let mut bytes = 0usize;
        for (index, path) in args.paths.iter().enumerate() {
            check(ctx)?;
            ctx.stage("reading");
            let size = ctx.file_size(path)?;
            if size > FILE_LIMIT as u64 || size > (TOTAL_LIMIT - bytes) as u64 {
                return Err(error("size").with("path", path.as_str()));
            }
            let mut reader = ctx.open_file(path)?.take(FILE_LIMIT as u64 + 1);
            let mut data = Vec::new();
            let mut chunk = [0; 64 * 1024];
            loop {
                check(ctx)?;
                let n = reader.read(&mut chunk).map_err(|_| error("read"))?;
                if n == 0 {
                    break;
                }
                if data.len() + n > FILE_LIMIT || bytes + n > TOTAL_LIMIT {
                    return Err(error("size"));
                }
                data.extend_from_slice(&chunk[..n]);
                bytes += n;
            }
            let text = String::from_utf8(data).map_err(|_| error("encoding"))?;
            ctx.stage("parsing");
            parser::parse(&mut report, path.clone(), &text, ctx)
                .map_err(|e| e.with("path", path.as_str()))?;
            ctx.progress(index as u64 + 1, args.paths.len() as u64);
        }
        check(ctx)?;
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.next += 1;
        let handle = cache.next.to_string();
        let summary = json!({"handle":handle,"counts":report.counts,"sourceCount":report.sources.len(),"suiteCount":report.suites.len(),"diagnosticCount":report.diagnostics.len()});
        ctx.log(
            LogLevel::Info,
            "loaded",
            json!({"count":report.cases.len()}),
        );
        cache.current = Some((handle, Arc::new(report)));
        Ok(summary)
    }
    fn detail(&self, args: DetailArgs) -> PluginResult<Value> {
        view::validate_page(args.offset, args.limit)?;
        let report = self.report(&args.handle)?;
        let (row, ids) = match args.kind.as_str() {
            "case" => {
                let c = report.cases.get(args.id).ok_or_else(|| error("page"))?;
                (to_value(c)?, &c.content)
            }
            "suite" => {
                let s = report.suites.get(args.id).ok_or_else(|| error("page"))?;
                (to_value(s)?, &s.content)
            }
            _ => return Err(error("page")),
        };
        let content: Vec<_> = ids
            .iter()
            .skip(args.offset)
            .take(args.limit)
            .map(|&id| view::content_meta(&report.content[id]))
            .collect();
        Ok(json!({"row":row,"total":ids.len(),"items":content}))
    }
    fn content(&self, args: ContentArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if args.limit == 0 || args.limit > 8192 || args.offset > TOTAL_LIMIT {
            return Err(error("page"));
        }
        let report = self.report(&args.handle)?;
        let c = report.content.get(args.id).ok_or_else(|| error("page"))?;
        let mut total = 0usize;
        let mut text = String::new();
        for ch in c.text.chars() {
            if total.is_multiple_of(8192) {
                check(ctx)?;
            }
            if (args.offset..args.offset + args.limit).contains(&total) {
                text.push(ch);
            }
            total += 1;
        }
        Ok(json!({"text":text,"total":total,"offset":args.offset,"limit":args.limit}))
    }
}
impl ToolPlugin for JunitViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, _: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        check(ctx)?;
        match function {
            "load" => self.load(parse_args(args)?, ctx),
            "browse" => {
                let args: view::BrowseArgs = parse_args(args)?;
                let report = self.report(&args.handle)?;
                view::browse(&report, &args, ctx)
            }
            "detail" => {
                let value = self.detail(parse_args(args)?)?;
                check(ctx)?;
                Ok(value)
            }
            "content" => self.content(parse_args(args)?, ctx),
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                ctx.stage("exporting");
                let report = self.report(&args.handle)?;
                let output = export::render(&report, &args.filter, &args.format, ctx)?;
                check(ctx)?;
                tf_plugin_guest::net::write_file(&args.path, output.as_bytes())?;
                Ok(json!({"saved":true,"bytes":output.len()}))
            }
            other => Err(unknown_function(other)),
        }
    }
}
tf_plugin_guest::export_plugin!(JunitViewer);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
