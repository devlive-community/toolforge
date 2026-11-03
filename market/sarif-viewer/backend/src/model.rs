use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

pub const FILE_LIMIT: usize = 16 * 1024 * 1024;
pub const TOTAL_LIMIT: usize = 32 * 1024 * 1024;
pub const ENTRY_LIMIT: usize = 50_000;
pub const RESULTS_LIMIT: usize = 20_000;
pub const PAGE: usize = 25;
pub const JSON_DEPTH_LIMIT: usize = 64;
pub const STRING_LIMIT: usize = 4096;
pub const SNIPPET_LIMIT: usize = 8192;
pub const RUNS_LIMIT: usize = 64;
pub const RULES_PER_RUN: usize = 2_000;
pub const ARTIFACTS_PER_RUN: usize = 5_000;
pub const BASES_PER_RUN: usize = 64;
pub const LOCATIONS_PER_RESULT: usize = 64;
pub const RELATED_PER_RESULT: usize = 64;
pub const FLOWS_PER_RESULT: usize = 32;
pub const THREADS_PER_FLOW: usize = 8;
pub const STEPS_PER_FLOW: usize = 128;
pub const SUPPRESSIONS_PER_RESULT: usize = 8;
pub const SOURCE_MAX_LINES: usize = 2_000;
pub const SOURCE_CONTEXT_MAX: usize = 32;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("sarif.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
    Note,
    None,
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub results: usize,
    pub errors: usize,
    pub warnings: usize,
    pub notes: usize,
    pub none: usize,
    pub suppressed: usize,
}
impl Counts {
    pub fn add_result(&mut self, row: &ResultRow) {
        self.results += 1;
        if row.suppressed {
            self.suppressed += 1;
        }
        match row.level {
            Level::Error => self.errors += 1,
            Level::Warning => self.warnings += 1,
            Level::Note => self.notes += 1,
            Level::None => self.none += 1,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub run: usize,
    pub result: Option<usize>,
    pub kind: String,
    pub detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub id: usize,
    pub path: String,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub id: usize,
    pub file: usize,
    pub tool: String,
    pub version: Option<String>,
    pub column_kind: String,
    pub artifact_count: usize,
    pub rule_count: usize,
    pub result_count: usize,
    pub extension_count: usize,
    pub external_properties: bool,
    pub invocation_success: Option<bool>,
    pub automation_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRow {
    pub id: usize,
    pub run: usize,
    pub rule_id: String,
    #[serde(skip)]
    pub messages: HashMap<String, String>,
    pub name: Option<String>,
    pub short_description: Option<String>,
    pub default_level: Option<Level>,
    pub help_uri: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRow {
    pub id: usize,
    pub run: usize,
    pub uri: String,
    pub uri_base_id: Option<String>,
    pub language: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseRow {
    pub run: usize,
    pub key: String,
    pub uri: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub start_line: Option<u64>,
    pub start_column: Option<u64>,
    pub end_line: Option<u64>,
    pub end_column: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationRow {
    pub id: usize,
    pub run: usize,
    pub result: usize,
    pub kind: String,
    pub artifact: Option<usize>,
    pub uri: Option<String>,
    pub uri_base_id: Option<String>,
    pub region: Option<Region>,
    pub snippet: Option<String>,
    pub logical: Option<String>,
    pub message: Option<String>,
    pub order: Option<u64>,
    pub nesting: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suppression {
    pub kind: String,
    pub status: Option<String>,
    pub justification: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRow {
    pub id: usize,
    pub run: usize,
    pub result: usize,
    pub message: Option<String>,
    pub steps: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultRow {
    pub id: usize,
    pub run: usize,
    pub rule: Option<usize>,
    pub rule_id: String,
    pub level: Level,
    pub kind: String,
    pub message: String,
    pub baseline_state: Option<String>,
    pub rank: Option<f64>,
    pub suppressed: bool,
    pub suppressions: Vec<Suppression>,
    pub occurrence_count: Option<u64>,
    #[serde(skip)]
    pub locations: Vec<usize>,
    #[serde(skip)]
    pub related: Vec<usize>,
    #[serde(skip)]
    pub flows: Vec<usize>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub runs: Vec<RunRow>,
    pub rules: Vec<RuleRow>,
    pub artifacts: Vec<ArtifactRow>,
    pub bases: Vec<BaseRow>,
    pub locations: Vec<LocationRow>,
    pub flows: Vec<FlowRow>,
    pub results: Vec<ResultRow>,
    pub files: Vec<FileRow>,
    pub diagnostics: Vec<Diagnostic>,
    pub counts: Counts,
}
impl Report {
    pub fn diagnostic(
        &mut self,
        run: usize,
        result: Option<usize>,
        kind: &str,
        detail: impl Into<String>,
    ) -> PluginResult<()> {
        if self.diagnostics.len() >= ENTRY_LIMIT {
            return Err(error("entries"));
        }
        self.diagnostics.push(Diagnostic {
            run,
            result,
            kind: kind.into(),
            detail: detail.into(),
        });
        Ok(())
    }
    pub fn budget(&self) -> PluginResult<()> {
        if self.rules.len()
            + self.artifacts.len()
            + self.locations.len()
            + self.flows.len()
            + self.results.len()
            + self.diagnostics.len()
            > ENTRY_LIMIT
        {
            Err(error("entries"))
        } else {
            Ok(())
        }
    }
    pub fn primary_location(&self, row: &ResultRow) -> Option<&LocationRow> {
        row.locations.first().map(|&id| &self.locations[id])
    }
}
