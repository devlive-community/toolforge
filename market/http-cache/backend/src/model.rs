use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tf_plugin_api::{PluginError, PluginResult, TaskContext};

pub const HEADER_BYTES: usize = 32768;
pub const FIELD_LIMIT: usize = 128;
pub const DELTA_LIMIT: u64 = 2_147_483_648;
pub fn error(code: &str) -> PluginError {
    PluginError::new(format!("http-cache.{code}"))
}
pub fn check(ctx: &dyn TaskContext) -> PluginResult<()> {
    if ctx.is_cancelled() {
        Err(tf_plugin_api::cancelled())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Private,
    Shared,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    pub role: Role,
    pub method: String,
    pub status: String,
    pub request_headers: String,
    pub response_headers: String,
    pub new_request_headers: Option<String>,
    pub request_time: String,
    pub response_time: String,
    pub now: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub code: String,
    pub params: Value,
}
impl Note {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            params: json!({}),
        }
    }
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.params[key] = value.into();
        self
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub name: String,
    pub value: String,
}
pub fn sensitive(name: &str) -> bool {
    [
        "authorization",
        "proxy-authorization",
        "cookie",
        "cookie2",
        "set-cookie",
        "set-cookie2",
        "authentication-info",
        "proxy-authentication-info",
        "x-api-key",
    ]
    .contains(&name)
}
pub fn masked(name: &str, value: &str) -> String {
    if sensitive(name) {
        "[redacted]".into()
    } else {
        value.to_owned()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Directive {
    pub context: String,
    pub name: String,
    pub value: Option<String>,
    pub valid: bool,
    pub supported: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    pub key: String,
    pub status: String,
    pub note: Note,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaryRow {
    pub name: String,
    pub stored: Option<String>,
    pub presented: Option<String>,
    pub status: String,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    pub date: Option<i64>,
    pub request_time: Option<i64>,
    pub response_time: Option<i64>,
    pub now: Option<i64>,
    pub age_value: u64,
    pub apparent_age: Option<u64>,
    pub response_delay: Option<u64>,
    pub corrected_age_value: Option<u64>,
    pub corrected_initial_age: Option<u64>,
    pub resident_time: Option<u64>,
    pub current_age: Option<u64>,
    pub lifetime: Option<u64>,
    pub lifetime_source: String,
    pub remaining: Option<i64>,
    pub freshness: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub name: String,
    pub role: Role,
    pub storage: String,
    pub reuse: String,
    pub freshness: String,
    pub lifetime: Option<u64>,
    pub current_age: Option<u64>,
    pub remaining: Option<i64>,
    pub lifetime_source: String,
    pub diagnostics: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseReport {
    pub summary: Summary,
    pub method: String,
    pub status: String,
    pub request_headers: Vec<Header>,
    pub response_headers: Vec<Header>,
    pub new_request_headers: Vec<Header>,
    pub same_request_assumed: bool,
    pub timing: Timing,
    pub conditions: Vec<Condition>,
    pub diagnostics: Vec<Note>,
    pub directives: Vec<Directive>,
    pub vary: Vec<VaryRow>,
    pub conditional: String,
    pub only_if_cached_unsatisfied: bool,
}

pub fn examples() -> Vec<Scenario> {
    let base = Scenario {name:"Baseline".into(), role:Role::Private, method:"GET".into(), status:"200".into(),
        request_headers:"Accept-Encoding: gzip\nAccept-Language: zh-CN".into(),
        response_headers:"Cache-Control: public, max-age=60, s-maxage=600\nDate: Thu, 22 Oct 2026 10:00:00 GMT\nAge: 20\nVary: Accept-Encoding\nETag: \"asset-v1\"\nLast-Modified: Wed, 21 Oct 2026 12:00:00 GMT".into(),
        new_request_headers:None,request_time:"2026-10-22T10:00:02Z".into(),response_time:"2026-10-22T10:00:04Z".into(),now:"2026-10-22T10:01:04Z".into()};
    let mut shared = base.clone();
    shared.role = Role::Shared;
    shared.name = "Experiment".into();
    vec![base, shared]
}
