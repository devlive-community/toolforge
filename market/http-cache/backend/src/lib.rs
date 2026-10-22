//! Offline cache analysis; no HTTP requests, response bodies or durable inputs.
mod engine;
mod export;
mod headers;
mod model;
mod timing;
use model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, unknown_function,
};

#[derive(Default)]
struct Cache {
    generation: u64,
    current: Option<(String, Arc<Vec<CaseReport>>)>,
}
pub struct HttpCache {
    manifest: Manifest,
    cache: Mutex<Cache>,
}
impl Default for HttpCache {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
            cache: Mutex::default(),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Inspect {
    cases: Vec<Scenario>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Query {
    handle: String,
    case: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_section")]
    section: String,
}
fn default_limit() -> usize {
    20
}
fn default_section() -> String {
    "conditions".into()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Page<'a, T> {
    items: Vec<&'a T>,
    offset: usize,
    limit: usize,
    total: usize,
}
fn page<T: Serialize>(items: &[T], query: &Query) -> PluginResult<Value> {
    let offset = query.offset.min(items.len().saturating_sub(1)) / query.limit * query.limit;
    tf_plugin_api::to_value(Page {
        items: items.iter().skip(offset).take(query.limit).collect(),
        offset,
        limit: query.limit,
        total: items.len(),
    })
}
impl HttpCache {
    fn reports(&self, handle: &str) -> PluginResult<Arc<Vec<CaseReport>>> {
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
impl ToolPlugin for HttpCache {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, _: Value) -> PluginResult<Value> {
        if function == "examples" {
            tf_plugin_api::to_value(examples())
        } else {
            Err(unknown_function(function))
        }
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        check(ctx)?;
        if function == "inspect" {
            let args: Inspect = parse_args(args)?;
            if args.cases.is_empty() || args.cases.len() > 2 {
                return Err(error("cases_limit"));
            }
            ctx.stage("parse");
            ctx.log(
                tf_plugin_api::LogLevel::Info,
                "analysis_started",
                json!({"count":args.cases.len()}),
            );
            let mut reports = Vec::new();
            for (i, case) in args.cases.iter().enumerate() {
                reports.push(engine::analyze(case, ctx)?);
                ctx.progress((i + 1) as u64, args.cases.len() as u64);
            }
            check(ctx)?;
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.generation = cache
                .generation
                .checked_add(1)
                .ok_or_else(|| error("expired"))?;
            let handle = cache.generation.to_string();
            let summaries: Vec<_> = reports.iter().map(|r| r.summary.clone()).collect();
            cache.current = Some((handle.clone(), Arc::new(reports)));
            ctx.log(
                tf_plugin_api::LogLevel::Info,
                "analysis_complete",
                json!({"count":summaries.len()}),
            );
            return Ok(json!({"handle":handle,"summaries":summaries}));
        }
        if function == "export" {
            let args: export::Args = parse_args(args)?;
            let reports = self.reports(&args.handle)?;
            return export::run(&reports, args, ctx);
        }
        if function != "query" {
            return Err(unknown_function(function));
        }
        let q: Query = parse_args(args)?;
        if q.limit == 0 || q.limit > 100 {
            return Err(error("page"));
        }
        ctx.stage("query");
        let reports = self.reports(&q.handle)?;
        let report = reports.get(q.case).ok_or_else(|| error("page"))?;
        let items = match q.section.as_str() {
            "conditions" => page(&report.conditions, &q)?,
            "diagnostics" => page(&report.diagnostics, &q)?,
            "directives" => page(&report.directives, &q)?,
            "vary" => page(&report.vary, &q)?,
            _ => return Err(error("page")),
        };
        check(ctx)?;
        Ok(
            json!({"handle":q.handle,"case":q.case,"section":q.section,"summary":report.summary,"timing":report.timing,"conditional":report.conditional,
            "sameRequestAssumed":report.same_request_assumed,"onlyIfCachedUnsatisfied":report.only_if_cached_unsatisfied,
            "headers":{"request":report.request_headers,"response":report.response_headers,"presented":report.new_request_headers},"page":items}),
        )
    }
}
tf_plugin_guest::export_plugin!(HttpCache);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
