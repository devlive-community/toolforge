//! Read-only esbuild metadata analysis. No source files or build tools are run.
mod analysis;
mod export;
mod model;
mod treemap;
use model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::{Arc, Mutex},
};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, unknown_function,
};
#[derive(Default)]
struct Cache {
    generation: u64,
    current: Option<(String, Arc<Reports>)>,
}
pub struct BundleAnalyzer {
    manifest: Manifest,
    cache: Mutex<Cache>,
}
impl Default for BundleAnalyzer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
            cache: Mutex::default(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Load {
    before_path: String,
    after_path: Option<String>,
}
fn read(path: &str, ctx: &dyn TaskContext) -> PluginResult<Snapshot> {
    if path.is_empty() || path.len() > 4096 {
        return Err(error("path"));
    }
    ctx.stage("read");
    if ctx.file_size(path)? > FILE_LIMIT as u64 {
        return Err(error("size"));
    }
    let mut reader = ctx.open_file(path)?.take(FILE_LIMIT as u64 + 1);
    let mut data = Vec::new();
    let mut chunk = [0; 65536];
    loop {
        check(ctx)?;
        let n = reader.read(&mut chunk).map_err(|_| error("file"))?;
        if n == 0 {
            break;
        }
        if data.len() + n > FILE_LIMIT {
            return Err(error("size"));
        }
        data.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8(data).map_err(|_| error("encoding"))?;
    ctx.stage("parse");
    let mut snapshot = model::parse(
        path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned(),
        &text,
        ctx,
    )?;
    snapshot.source = path.to_owned();
    Ok(snapshot)
}
fn summary(s: &Snapshot) -> Value {
    json!({"name":s.name,"totals":s.totals,"diagnostics":s.diagnostics.len(),"firstOutput":s.meta.outputs.keys().next()})
}
impl BundleAnalyzer {
    fn reports(&self, handle: &str) -> PluginResult<Arc<Reports>> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current
            .as_ref()
            .filter(|(h, _)| h == handle)
            .map(|(_, r)| r.clone())
            .ok_or_else(|| error("expired"))
    }
}
impl ToolPlugin for BundleAnalyzer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, _: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        check(ctx)?;
        if function == "load" {
            let args: Load = parse_args(args)?;
            let before = read(&args.before_path, ctx)?;
            ctx.progress(1, if args.after_path.is_some() { 2 } else { 1 });
            let after = args.after_path.map(|p| read(&p, ctx)).transpose()?;
            check(ctx)?;
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.generation += 1;
            let handle = cache.generation.to_string();
            let result = json!({"handle":handle,"before":summary(&before),"after":after.as_ref().map(summary),"totalDelta":after.as_ref().map(|s|s.totals.bytes as i64-before.totals.bytes as i64)});
            cache.current = Some((handle, Arc::new(Reports { before, after })));
            return Ok(result);
        }
        if function == "export" || function == "copy" {
            let args: export::Request = parse_args(args)?;
            let reports = self.reports(&args.query.handle)?;
            return export::run(&reports, args, function == "copy", ctx);
        }
        if function != "query" && function != "trace" {
            return Err(unknown_function(function));
        }
        let query: analysis::Query = parse_args(args)?;
        query.validate()?;
        let reports = self.reports(&query.handle)?;
        ctx.stage("analyze");
        let result = if function == "trace" {
            analysis::trace(
                reports.side(&query.side)?,
                query.output.as_deref().ok_or_else(|| error("output"))?,
                query.module.as_deref().ok_or_else(|| error("module"))?,
                ctx,
            )?
        } else if query.kind == "tree" {
            treemap::tree(reports.side(&query.side)?, &query, ctx)?
        } else {
            let rows = analysis::rows(&reports, &query, ctx)?;
            json!({"total":rows.len(),"items":rows.into_iter().skip(query.offset).take(query.limit).collect::<Vec<_>>()})
        };
        check(ctx)?;
        Ok(result)
    }
}
tf_plugin_guest::export_plugin!(BundleAnalyzer);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
