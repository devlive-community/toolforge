//! Offline SARIF inspection. Reports stay in Rust; the UI receives projected,
//! paginated views and explicit source context only on request.
mod export;
mod model;
mod parser;
mod source;
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
pub struct SarifViewer {
    manifest: Manifest,
    cache: Mutex<Cache>,
}
impl Default for SarifViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
            cache: Mutex::default(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ParseArgs {
    paths: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetailArgs {
    handle: String,
    id: usize,
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
impl SarifViewer {
    fn report(&self, handle: &str) -> PluginResult<Arc<Report>> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .current
            .as_ref()
            .filter(|(id, _)| id == handle)
            .map(|(_, r)| r.clone())
            .ok_or_else(|| error("expired"))
    }
    fn parse(&self, args: ParseArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
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
        let summary = json!({
            "handle":handle,
            "counts":report.counts,
            "fileCount":report.files.len(),
            "runCount":report.runs.len(),
            "ruleCount":report.rules.len(),
            "artifactCount":report.artifacts.len(),
            "diagnosticCount":report.diagnostics.len(),
        });
        ctx.log(
            LogLevel::Info,
            "loaded",
            json!({"results":report.counts.results}),
        );
        cache.current = Some((handle, Arc::new(report)));
        Ok(summary)
    }
    fn detail(&self, args: DetailArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let report = self.report(&args.handle)?;
        let row = report.results.get(args.id).ok_or_else(|| error("page"))?;
        let locations: Vec<_> = row
            .locations
            .iter()
            .map(|&id| to_value(&report.locations[id]))
            .collect::<PluginResult<Vec<_>>>()?;
        let related: Vec<_> = row
            .related
            .iter()
            .map(|&id| to_value(&report.locations[id]))
            .collect::<PluginResult<Vec<_>>>()?;
        let flows: Vec<_> = row
            .flows
            .iter()
            .map(|&id| -> PluginResult<Value> {
                let flow = &report.flows[id];
                let steps = flow
                    .steps
                    .iter()
                    .map(|&id| to_value(&report.locations[id]))
                    .collect::<PluginResult<Vec<_>>>()?;
                Ok(json!({"id":flow.id,"message":flow.message,"steps":steps}))
            })
            .collect::<PluginResult<Vec<_>>>()?;
        let rule = row.rule.map(|id| to_value(&report.rules[id])).transpose()?;
        let diagnostics: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.result == Some(args.id))
            .collect();
        check(ctx)?;
        let column_kind = report.runs[row.run].column_kind.clone();
        Ok(json!({
            "row":row,
            "rule":rule,
            "columnKind":column_kind,
            "locations":locations,
            "related":related,
            "flows":flows,
            "diagnostics":diagnostics,
        }))
    }
}
impl ToolPlugin for SarifViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, _: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        check(ctx)?;
        match function {
            "parse" => self.parse(parse_args(args)?, ctx),
            "browse" => {
                let args: view::BrowseArgs = parse_args(args)?;
                let report = self.report(&args.handle)?;
                view::browse(&report, &args, ctx)
            }
            "detail" => self.detail(parse_args(args)?, ctx),
            "readSource" => {
                let args: source::ReadSourceArgs = parse_args(args)?;
                let report = self.report(&args.handle)?;
                ctx.stage("sourcing");
                source::read(&report, &args, ctx)
            }
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
tf_plugin_guest::export_plugin!(SarifViewer);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
