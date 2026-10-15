use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::http::{BodyKind, Pair, Request};

const MAX_WORKSPACE: usize = 1536 * 1024;
const MAX_REQUEST: usize = 1024 * 1024;
const MAX_TEXT: usize = 512 * 1024;
const MAX_URL: usize = 64 * 1024;
const MAX_PAIRS: usize = 256;
const MAX_VARIABLES: usize = 200;

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Variable {
    pub key: String,
    pub value: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Collection {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedRequest {
    pub id: String,
    pub name: String,
    pub collection_id: Option<String>,
    pub request: Request,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workspace {
    pub version: u32,
    pub collections: Vec<Collection>,
    pub requests: Vec<SavedRequest>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceArgs {
    pub workspace: Workspace,
}

#[derive(Deserialize)]
pub struct RequestArgs {
    #[serde(flatten)]
    pub request: Request,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveArgs {
    pub request: Request,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

pub fn new_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("http-{epoch:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}

fn limit(field: &str, value: usize, maximum: usize) -> PluginResult<()> {
    if value > maximum {
        Err(PluginError::new("http.limit_exceeded")
            .with("field", field)
            .with("limit", maximum))
    } else {
        Ok(())
    }
}

fn serialized_limit(value: &impl Serialize, field: &str, maximum: usize) -> PluginResult<()> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| PluginError::new("plugin.serialize_failed"))?;
    limit(field, bytes.len(), maximum)
}

fn named(value: &str, field: &str, maximum: usize) -> PluginResult<()> {
    limit(field, value.len(), maximum)?;
    if value.trim().is_empty() {
        return Err(PluginError::new("http.workspace_empty_field").with("field", field));
    }
    Ok(())
}

fn pairs(pairs: &[Pair], field: &str) -> PluginResult<()> {
    limit(field, pairs.len(), MAX_PAIRS)?;
    for pair in pairs {
        limit(field, pair.key.len(), MAX_URL)?;
        limit(field, pair.value.len(), MAX_TEXT)?;
    }
    Ok(())
}

pub fn validate_request(request: &Request) -> PluginResult<()> {
    named(&request.method, "method", 32)?;
    limit("url", request.url.len(), MAX_URL)?;
    pairs(&request.params, "params")?;
    pairs(&request.headers, "headers")?;
    pairs(&request.body.form, "body.form")?;
    limit("body.text", request.body.text.len(), MAX_TEXT)?;
    serialized_limit(request, "request", MAX_REQUEST)
}

fn variable_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

fn validate_variables(variables: &[Variable]) -> PluginResult<()> {
    limit("variables", variables.len(), MAX_VARIABLES)?;
    let mut keys = HashSet::new();
    for variable in variables {
        limit("variable.key", variable.key.len(), 128)?;
        limit("variable.value", variable.value.len(), MAX_TEXT)?;
        if !variable_key(&variable.key) {
            return Err(PluginError::new("http.variable_key").with("field", "variable.key"));
        }
        if !keys.insert(variable.key.as_str()) {
            return Err(
                PluginError::new("http.variable_duplicate").with("key", variable.key.as_str())
            );
        }
    }
    serialized_limit(&variables, "variables", MAX_REQUEST)
}

/// Validate the in-memory workspace before removing secret values from the returned copy.
pub fn validate(mut workspace: Workspace) -> PluginResult<Workspace> {
    if workspace.version != 1 {
        return Err(PluginError::new("http.workspace_version").with("version", workspace.version));
    }
    limit("collections", workspace.collections.len(), 100)?;
    limit("requests", workspace.requests.len(), 200)?;
    limit("environments", workspace.environments.len(), 50)?;
    serialized_limit(&workspace, "workspace", MAX_WORKSPACE)?;

    let mut ids = HashSet::new();
    let mut register = |id: &str, name: &str, field: &str| -> PluginResult<()> {
        named(id, field, 128)?;
        named(name, "name", 256)?;
        if !ids.insert(id.to_owned()) {
            return Err(PluginError::new("http.workspace_duplicate_id").with("field", field));
        }
        Ok(())
    };
    for collection in &workspace.collections {
        register(&collection.id, &collection.name, "collection.id")?;
    }
    for request in &workspace.requests {
        register(&request.id, &request.name, "request.id")?;
        validate_request(&request.request)?;
        if let Some(id) = &request.collection_id
            && !workspace.collections.iter().any(|item| item.id == *id)
        {
            return Err(PluginError::new("http.workspace_reference").with("field", "collectionId"));
        }
    }
    for environment in &mut workspace.environments {
        register(&environment.id, &environment.name, "environment.id")?;
        validate_variables(&environment.variables)?;
        for variable in &mut environment.variables {
            if variable.secret {
                variable.value.clear();
            }
        }
    }
    if let Some(id) = &workspace.active_environment_id
        && !workspace.environments.iter().any(|item| item.id == *id)
    {
        return Err(
            PluginError::new("http.workspace_reference").with("field", "activeEnvironmentId")
        );
    }
    Ok(workspace)
}

fn replace(
    text: &str,
    variables: &HashMap<&str, &str>,
    field: &str,
    maximum: usize,
) -> PluginResult<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .ok_or_else(|| PluginError::new("http.variable_syntax").with("field", field))?;
        let key = after[..end].trim();
        if !variable_key(key) {
            return Err(PluginError::new("http.variable_syntax").with("field", field));
        }
        let value = variables
            .get(key)
            .ok_or_else(|| PluginError::new("http.variable_missing").with("key", key))?;
        limit(field, out.len().saturating_add(value.len()), maximum)?;
        out.push_str(value);
        rest = &after[end + 2..];
    }
    limit(field, out.len().saturating_add(rest.len()), maximum)?;
    out.push_str(rest);
    Ok(out)
}

fn replace_pairs(pairs: &mut [Pair], vars: &HashMap<&str, &str>, field: &str) -> PluginResult<()> {
    for pair in pairs.iter_mut().filter(|pair| pair.enabled) {
        pair.key = replace(&pair.key, vars, field, MAX_URL)?;
        pair.value = replace(&pair.value, vars, field, MAX_TEXT)?;
    }
    Ok(())
}

/// Substitution is a single pass over the template. Inserted values are never interpreted again.
pub fn resolve(mut request: Request, variables: &[Variable]) -> PluginResult<Request> {
    validate_request(&request)?;
    validate_variables(variables)?;
    let vars: HashMap<_, _> = variables
        .iter()
        .filter(|variable| variable.enabled)
        .map(|variable| (variable.key.as_str(), variable.value.as_str()))
        .collect();
    request.url = replace(&request.url, &vars, "url", MAX_URL)?;
    replace_pairs(&mut request.params, &vars, "params")?;
    replace_pairs(&mut request.headers, &vars, "headers")?;
    match request.body.kind {
        BodyKind::Json | BodyKind::Text => {
            request.body.text = replace(&request.body.text, &vars, "body.text", MAX_TEXT)?;
        }
        BodyKind::Form => replace_pairs(&mut request.body.form, &vars, "body.form")?,
        BodyKind::None => {}
    }
    validate_request(&request)?;
    Ok(request)
}

impl RequestArgs {
    pub fn resolve(&self) -> PluginResult<Request> {
        resolve(self.request.clone(), &self.variables)
    }

    pub fn protect_error(&self, mut error: PluginError) -> PluginError {
        if self
            .variables
            .iter()
            .any(|variable| variable.enabled && variable.secret)
        {
            // TaskManager persists error parameters, including errors containing encoded URLs.
            for value in error.params.values_mut().filter(|value| value.is_string()) {
                *value = serde_json::Value::String("[redacted]".into());
            }
        }
        error
    }
}

#[cfg(test)]
#[path = "workspace_test.rs"]
mod tests;
