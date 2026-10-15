use std::time::{Duration, Instant};

use jsonschema::error::ValidationErrorKind;
use jsonschema::{PatternOptions, Retrieve, Uri, ValidationError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

use crate::guard;

const MAX_DIAGNOSTICS: usize = 200;
const MAX_MESSAGE_CHARS: usize = 1_024;
const TIME_BUDGET: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub enum Draft {
    #[default]
    #[serde(rename = "2020-12")]
    Draft202012,
    #[serde(rename = "7")]
    Draft7,
}

impl Draft {
    pub fn matches_uri(self, uri: &str) -> bool {
        match self {
            Self::Draft202012 => {
                uri.trim_end_matches('#') == "https://json-schema.org/draft/2020-12/schema"
            }
            Self::Draft7 => matches!(
                uri.trim_end_matches('#'),
                "http://json-schema.org/draft-07/schema"
                    | "https://json-schema.org/draft-07/schema"
            ),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub instance: String,
    pub schema: String,
    #[serde(default)]
    pub draft: Draft,
    #[serde(default)]
    pub validate_formats: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub target: &'static str,
    pub code: &'static str,
    pub instance_path: String,
    pub schema_path: String,
    pub keyword: String,
    pub detail: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub valid: bool,
    pub schema_valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

struct DenyRetrieval;

impl Retrieve for DenyRetrieval {
    fn retrieve(
        &self,
        _uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("External references are disabled; use a fragment in this document".into())
    }
}

fn checkpoint(started: Instant, ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        return Err(cancelled());
    }
    if started.elapsed() > TIME_BUDGET {
        return Err(PluginError::new("schema.time_budget").with("seconds", 10));
    }
    Ok(())
}

fn short(message: impl std::fmt::Display) -> String {
    message
        .to_string()
        .chars()
        .take(MAX_MESSAGE_CHARS)
        .collect()
}

fn diagnostic(error: &ValidationError<'_>, target: &'static str) -> Diagnostic {
    let instance_path = error.instance_path().to_string();
    let schema_path = error.schema_path().to_string();
    Diagnostic {
        target,
        code: match error.kind() {
            ValidationErrorKind::BacktrackLimitExceeded { .. } => "schema.regex_budget",
            ValidationErrorKind::RegexEngineFailure { .. } => "schema.regex_failure",
            _ if target == "schema" => "schema.invalid_schema",
            _ => "schema.validation",
        },
        keyword: schema_path
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_owned(),
        instance_path,
        schema_path,
        detail: short(error.masked()),
        line: None,
        column: None,
    }
}

fn parse(text: &str, target: &'static str) -> Result<Value, Box<Diagnostic>> {
    serde_json::from_str(text).map_err(|error| {
        Box::new(Diagnostic {
            target,
            code: "schema.invalid_json",
            instance_path: String::new(),
            schema_path: String::new(),
            keyword: String::new(),
            detail: short(&error),
            line: Some(error.line()),
            column: Some(error.column()),
        })
    })
}

fn collect<'a>(
    errors: impl Iterator<Item = ValidationError<'a>>,
    target: &'static str,
    started: Instant,
    ctx: &dyn TaskContext,
) -> PluginResult<(Vec<Diagnostic>, bool)> {
    let mut diagnostics = Vec::new();
    for error in errors {
        checkpoint(started, ctx)?;
        if diagnostics.len() == MAX_DIAGNOSTICS {
            return Ok((diagnostics, true));
        }
        diagnostics.push(diagnostic(&error, target));
    }
    Ok((diagnostics, false))
}

pub fn run(args: Args, ctx: &dyn TaskContext) -> PluginResult<Report> {
    let started = Instant::now();
    checkpoint(started, ctx)?;
    ctx.stage("schema.parse");
    guard::text(&args.instance, "instance")?;
    guard::text(&args.schema, "schema")?;
    let mut report = Report {
        valid: false,
        schema_valid: false,
        diagnostics: Vec::new(),
        truncated: false,
        elapsed_ms: 0,
    };
    let schema = parse(&args.schema, "schema");
    let instance = parse(&args.instance, "instance");
    if let Err(error) = &schema {
        report.diagnostics.push(error.as_ref().clone());
    }
    if let Err(error) = &instance {
        report.diagnostics.push(error.as_ref().clone());
    }
    let (Ok(schema), Ok(instance)) = (schema, instance) else {
        report.elapsed_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    };
    guard::nodes(&schema, "schema")?;
    guard::nodes(&instance, "instance")?;
    if let Err(issue) = guard::schema(&schema, args.draft) {
        report.diagnostics.push(Diagnostic {
            target: "schema",
            code: issue.code,
            instance_path: issue.path,
            schema_path: String::new(),
            keyword: String::new(),
            detail: issue.detail,
            line: None,
            column: None,
        });
        report.elapsed_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    }
    checkpoint(started, ctx)?;
    ctx.stage("schema.compile");
    // The engine's error iterator eagerly builds its complete error vector.
    // Bound expanded work and path storage before entering that implementation.
    crate::work::check(&schema, &instance, args.draft)?;
    checkpoint(started, ctx)?;
    let meta = match args.draft {
        Draft::Draft202012 => jsonschema::draft202012::meta::validator(),
        Draft::Draft7 => jsonschema::draft7::meta::validator(),
    };
    let (diagnostics, truncated) = collect(meta.iter_errors(&schema), "schema", started, ctx)?;
    checkpoint(started, ctx)?;
    if !diagnostics.is_empty() {
        report.diagnostics = diagnostics;
        report.truncated = truncated;
        report.elapsed_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    }
    let draft = match args.draft {
        Draft::Draft202012 => jsonschema::Draft::Draft202012,
        Draft::Draft7 => jsonschema::Draft::Draft7,
    };
    // Feature flags, a rejecting retriever and reference preflight are separate
    // safeguards: `$id` cannot turn a fragment into implicit network/file I/O.
    let validator = jsonschema::options()
        .with_draft(draft)
        .with_retriever(DenyRetrieval)
        .should_validate_formats(args.validate_formats)
        .with_pattern_options(
            PatternOptions::fancy_regex()
                .backtrack_limit(10_000)
                .size_limit(1024 * 1024)
                .dfa_size_limit(1024 * 1024),
        )
        .build(&schema);
    checkpoint(started, ctx)?;
    match validator {
        Err(error) => report.diagnostics.push(diagnostic(&error, "schema")),
        Ok(validator) => {
            report.schema_valid = true;
            ctx.stage("schema.validate");
            let (diagnostics, truncated) =
                collect(validator.iter_errors(&instance), "instance", started, ctx)?;
            checkpoint(started, ctx)?;
            report.valid = diagnostics.is_empty();
            report.diagnostics = diagnostics;
            report.truncated = truncated;
        }
    }
    report.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

#[cfg(test)]
#[path = "validate_test.rs"]
mod tests;
