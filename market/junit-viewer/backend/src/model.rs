use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, TaskContext, cancelled};

pub const FILE_LIMIT: usize = 16 * 1024 * 1024;
pub const TOTAL_LIMIT: usize = 32 * 1024 * 1024;
pub const ENTRY_LIMIT: usize = 50_000;
pub const PAGE: usize = 25;

pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("junit.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Passed,
    Failure,
    Error,
    Skipped,
    Unknown,
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub tests: usize,
    pub passed: usize,
    pub failures: usize,
    pub errors: usize,
    pub skipped: usize,
    pub unknown: usize,
    pub known_seconds: f64,
    pub missing_time: usize,
}
impl Counts {
    pub fn add_case(&mut self, case: &Case) {
        self.tests += 1;
        match case.status {
            Status::Passed => self.passed += 1,
            Status::Failure => self.failures += 1,
            Status::Error => self.errors += 1,
            Status::Skipped => self.skipped += 1,
            Status::Unknown => self.unknown += 1,
        }
        match case.seconds {
            Some(t) => self.known_seconds += t,
            None => self.missing_time += 1,
        }
    }
    pub fn add(&mut self, rhs: &Self) {
        self.tests += rhs.tests;
        self.passed += rhs.passed;
        self.failures += rhs.failures;
        self.errors += rhs.errors;
        self.skipped += rhs.skipped;
        self.unknown += rhs.unknown;
        self.known_seconds += rhs.known_seconds;
        self.missing_time += rhs.missing_time;
    }
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Declared {
    pub tests: Option<usize>,
    pub failures: Option<usize>,
    pub errors: Option<usize>,
    pub skipped: Option<usize>,
    pub seconds: Option<f64>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub source: usize,
    pub suite: Option<usize>,
    pub case: Option<usize>,
    pub kind: String,
    pub detail: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: usize,
    pub path: String,
    pub name: String,
    pub counts: Counts,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suite {
    pub id: usize,
    pub source: usize,
    pub parent: Option<usize>,
    pub name: String,
    pub kind: String,
    pub declared: Declared,
    pub counts: Counts,
    #[serde(skip)]
    pub content: Vec<usize>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: usize,
    pub source: usize,
    pub suite: usize,
    pub name: String,
    pub classname: String,
    pub status: Status,
    pub seconds: Option<f64>,
    pub file: Option<String>,
    pub line: Option<String>,
    #[serde(skip)]
    pub content: Vec<usize>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub id: usize,
    pub source: usize,
    pub suite: usize,
    pub case: Option<usize>,
    pub kind: String,
    pub message: Option<String>,
    pub type_name: Option<String>,
    pub text: String,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub sources: Vec<Source>,
    pub suites: Vec<Suite>,
    pub cases: Vec<Case>,
    pub content: Vec<Content>,
    pub diagnostics: Vec<Diagnostic>,
    pub counts: Counts,
}
impl Report {
    pub fn diagnostic(
        &mut self,
        source: usize,
        suite: Option<usize>,
        case: Option<usize>,
        kind: &str,
        detail: impl Into<String>,
    ) -> PluginResult<()> {
        if self.diagnostics.len() >= ENTRY_LIMIT {
            return Err(error("entries"));
        }
        self.diagnostics.push(Diagnostic {
            source,
            suite,
            case,
            kind: kind.into(),
            detail: detail.into(),
        });
        Ok(())
    }
    pub fn budget(&self) -> PluginResult<()> {
        if self.suites.len() + self.cases.len() + self.content.len() > ENTRY_LIMIT {
            Err(error("entries"))
        } else {
            Ok(())
        }
    }
}
