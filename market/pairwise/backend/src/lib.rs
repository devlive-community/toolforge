//! Bounded exhaustive feasibility plus seeded greedy pair coverage. Models are
//! persisted by the UI through the host's SQLite plugin state, never a new DB.
mod model;
mod solver;

use std::collections::{BTreeMap, VecDeque};
use std::io::Read;
use std::sync::{Arc, Mutex};

use model::{
    ALGORITHM, Compiled, Draft, MAX_INPUT, Model, error, from_draft, normalize, parse_json,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use solver::{Limits, Report};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    text: Option<String>,
    draft: Option<Draft>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bundle {
    format_version: u32,
    algorithm: String,
    seed: String,
    model: Model,
    limits: Limits,
    cases: Vec<BTreeMap<String, Value>>,
    evidence: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateArgs {
    text: Option<String>,
    draft: Option<Draft>,
    path: Option<String>,
    #[serde(default)]
    seed: String,
    #[serde(default)]
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PageArgs {
    handle: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "page_size")]
    limit: usize,
    pair_id: Option<usize>,
    state: Option<String>,
}

fn page_size() -> usize {
    25
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportArgs {
    handle: String,
    format: String,
    path: String,
    #[serde(default)]
    allow_partial: bool,
}

#[derive(Default)]
struct Cache {
    next: u64,
    reports: VecDeque<(String, Arc<Report>)>,
}

pub struct Pairwise {
    manifest: Manifest,
    cache: Mutex<Cache>,
}

impl Default for Pairwise {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            cache: Mutex::default(),
        }
    }
}

enum Document {
    Model(Compiled),
    Bundle(Compiled, Box<Bundle>),
}

fn document(input: Input) -> PluginResult<Document> {
    match (input.text, input.draft) {
        (None, Some(draft)) => Ok(Document::Model(normalize(from_draft(draft)?)?)),
        (Some(text), None) => {
            let value: Value = parse_json(&text)?;
            if value.get("formatVersion").is_some() {
                let bundle: Bundle = parse_json(&text)?;
                if bundle.format_version != 1 || bundle.algorithm != ALGORITHM {
                    return Err(error("version"));
                }
                Ok(Document::Bundle(
                    normalize(bundle.model.clone())?,
                    Box::new(bundle),
                ))
            } else {
                Ok(Document::Model(normalize(parse_json(&text)?)?))
            }
        }
        _ => Err(error("input")),
    }
}

fn read_input(path: &str, ctx: &dyn TaskContext) -> PluginResult<String> {
    if ctx.file_size(path)? > MAX_INPUT as u64 {
        return Err(error("input_limit"));
    }
    let mut bytes = Vec::new();
    ctx.open_file(path)?
        .take(MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("file"))?;
    if bytes.len() > MAX_INPUT {
        return Err(error("input_limit"));
    }
    String::from_utf8(bytes).map_err(|_| error("encoding"))
}

impl Pairwise {
    fn report(&self, handle: &str) -> PluginResult<Arc<Report>> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reports
            .iter()
            .find(|(id, _)| id == handle)
            .map(|(_, r)| r.clone())
            .ok_or_else(|| error("expired"))
    }

    fn generate(&self, args: GenerateArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        ctx.stage("parse");
        let input = if let Some(path) = args.path {
            if args.text.is_some() || args.draft.is_some() {
                return Err(error("input"));
            }
            Input {
                text: Some(read_input(&path, ctx)?),
                draft: None,
            }
        } else {
            Input {
                text: args.text,
                draft: args.draft,
            }
        };
        let report = match document(input)? {
            Document::Model(compiled) => {
                solver::solve(compiled, args.seed, args.limits, None, ctx)?
            }
            Document::Bundle(compiled, bundle) => {
                let bundle = *bundle;
                // Exported coverage claims are never trusted: enumerate again
                // and validate the imported cases independently.
                let cases = solver::imported_cases(&compiled, bundle.cases)?;
                let mut limits = bundle.limits;
                limits.budget_ms = args.limits.budget_ms;
                solver::solve(compiled, bundle.seed, limits, Some(cases), ctx)?
            }
        };
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.next += 1;
        let handle = cache.next.to_string();
        let summary = to_value(report.summary(handle.clone()))?;
        cache.reports.push_back((handle, Arc::new(report)));
        // Bounded transient report cache, not persistent user history.
        while cache.reports.len() > 3 {
            cache.reports.pop_front();
        }
        Ok(summary)
    }

    fn page(&self, args: PageArgs, pairs: bool) -> PluginResult<Value> {
        if args.limit == 0 || args.limit > 100 || args.offset > 100_000 {
            return Err(error("page"));
        }
        let report = self.report(&args.handle)?;
        if pairs {
            if let Some(state) = &args.state
                && !["covered", "uncovered", "infeasible", "unknown"].contains(&state.as_str())
            {
                return Err(error("page"));
            }
            let rows = report
                .pairs
                .iter()
                .enumerate()
                .filter(|(_, p)| args.state.as_ref().is_none_or(|s| s == p.state));
            let total = rows.clone().count();
            let items = rows.skip(args.offset).take(args.limit).map(|(id, p)| {
                let left = &report.compiled.model.factors[p.left];
                let right = &report.compiled.model.factors[p.right];
                json!({ "id": id, "left": left.name, "leftValue": left.values[p.left_value].to_string(),
                    "right": right.name, "rightValue": right.values[p.right_value].to_string(),
                    "state": p.state, "caseCount": p.cases.len() })
            }).collect::<Vec<_>>();
            Ok(json!({ "total": total, "items": items }))
        } else {
            let ids: Vec<usize> = match args.pair_id {
                Some(id) => report
                    .pairs
                    .get(id)
                    .ok_or_else(|| error("page"))?
                    .cases
                    .clone(),
                None => (1..=report.cases.len()).collect(),
            };
            let items = ids
                .iter()
                .skip(args.offset)
                .take(args.limit)
                .map(|&id| {
                    let cells = report.cases[id - 1]
                        .iter()
                        .enumerate()
                        .map(|(f, &v)| report.compiled.model.factors[f].values[v].to_string())
                        .collect::<Vec<_>>();
                    json!({ "id": id, "cells": cells })
                })
                .collect::<Vec<_>>();
            Ok(json!({ "total": ids.len(), "items": items }))
        }
    }

    fn export(&self, args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let report = self.report(&args.handle)?;
        if args.path.trim().is_empty() {
            return Err(error("file"));
        }
        if report.status != "complete" && (args.format != "json" || !args.allow_partial) {
            return Err(error("partial_export"));
        }
        ctx.stage("export");
        let content = match args.format.as_str() {
            "json" => serde_json::to_string_pretty(&Bundle {
                format_version: 1,
                algorithm: ALGORITHM.to_owned(),
                seed: report.seed.clone(),
                model: report.compiled.model.clone(),
                limits: report.limits.clone(),
                cases: report.case_maps(),
                evidence: to_value(report.summary(String::new()))?,
            })
            .map_err(|_| error("file"))?,
            "csv" => csv(&report),
            _ => return Err(error("format")),
        };
        if content.len() > MAX_INPUT {
            return Err(error("output_limit"));
        }
        if ctx.is_cancelled() {
            return Err(tf_plugin_api::cancelled());
        }
        tf_plugin_guest::net::write_file(&args.path, content.as_bytes())?;
        Ok(json!({ "saved": true }))
    }
}

fn csv(report: &Report) -> String {
    fn cell(text: &str) -> String {
        format!("\"{}\"", text.replace('"', "\"\""))
    }
    let mut lines = vec![
        report
            .compiled
            .model
            .factors
            .iter()
            .map(|f| {
                let name = if f.name.trim_start().starts_with(['=', '+', '-', '@']) {
                    format!("'{}", f.name)
                } else {
                    f.name.clone()
                };
                cell(&name)
            })
            .collect::<Vec<_>>()
            .join(","),
    ];
    for row in &report.cases {
        // JSON scalar syntax preserves type and avoids spreadsheet formula
        // interpretation of user strings beginning with =, +, - or @.
        lines.push(
            row.iter()
                .enumerate()
                .map(|(f, &v)| cell(&report.compiled.model.factors[f].values[v].to_string()))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    lines.join("\r\n") + "\r\n"
}

impl ToolPlugin for Pairwise {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "prepare" => {
                let input: Input = parse_args(args)?;
                let compiled = match document(input)? {
                    Document::Model(c) | Document::Bundle(c, _) => c,
                };
                Ok(compiled.draft())
            }
            "cases" => self.page(parse_args(args)?, false),
            "pairs" => self.page(parse_args(args)?, true),
            other => Err(unknown_function(other)),
        }
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "generate" => self.generate(parse_args(args)?, ctx),
            "export" => self.export(parse_args(args)?, ctx),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(Pairwise);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
