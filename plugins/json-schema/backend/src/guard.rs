//! Checks run before compilation. No user-controlled URI is ever retrieved.

use std::collections::HashSet;

use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::validate::Draft;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 50_000;
const MAX_SCHEMA_NODES: usize = 4_096;
const MAX_PATTERN_BYTES: usize = 4_096;
const MAX_PATTERNS: usize = 128;
const MAX_BRANCHES: usize = 256;
const MAX_NUMBER_BYTES: usize = 4_096;
const MAX_EXPONENT: u64 = 10_000;
const MAX_KEY_BYTES: usize = 512;
const MAX_PATH_BYTES: usize = 2_048;

/// A byte scan protects the parser before it allocates nested values. Brackets
/// inside strings are ignored, including escaped quotes and backslashes.
pub fn text(text: &str, target: &str) -> PluginResult<()> {
    if text.len() > MAX_BYTES {
        return Err(PluginError::new("schema.too_large")
            .with("target", target)
            .with("mb", 2));
    }
    let (mut depth, mut string, mut escaped) = (0usize, false, false);
    for byte in text.bytes() {
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
        } else {
            match byte {
                b'"' => string = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > MAX_DEPTH {
                        return Err(PluginError::new("schema.too_deep")
                            .with("target", target)
                            .with("depth", MAX_DEPTH as u64));
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

pub fn nodes(value: &Value, target: &str) -> PluginResult<()> {
    let mut pending = vec![(value, 0usize)];
    let mut count = 0;
    while let Some((value, path_bytes)) = pending.pop() {
        count += 1;
        if count > MAX_NODES {
            return Err(PluginError::new("schema.node_budget")
                .with("target", target)
                .with("limit", MAX_NODES as u64));
        }
        match value {
            Value::Array(items) => {
                for (index, value) in items.iter().enumerate() {
                    let child_path_bytes = path_bytes + 1 + index.to_string().len();
                    if child_path_bytes > MAX_PATH_BYTES {
                        return Err(PluginError::new("schema.path_budget").with("target", target));
                    }
                    pending.push((value, child_path_bytes));
                }
            }
            Value::Object(map) => {
                for (key, value) in map {
                    let child_path_bytes = path_bytes
                        + 1
                        + key.len()
                        + key
                            .bytes()
                            .filter(|byte| matches!(byte, b'~' | b'/'))
                            .count();
                    if key.len() > MAX_KEY_BYTES || child_path_bytes > MAX_PATH_BYTES {
                        return Err(PluginError::new("schema.path_budget").with("target", target));
                    }
                    pending.push((value, child_path_bytes));
                }
            }
            Value::Number(number) => {
                let raw = number.to_string();
                let exponent_exceeds_budget =
                    raw.split_once(['e', 'E']).is_some_and(|(_, exponent)| {
                        exponent
                            .parse::<i64>()
                            .map_or(true, |value| value.unsigned_abs() > MAX_EXPONENT)
                    });
                if raw.len() > MAX_NUMBER_BYTES || exponent_exceeds_budget {
                    return Err(PluginError::new("schema.numeric_budget").with("target", target));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}

#[derive(Debug)]
pub struct Issue {
    pub code: &'static str,
    pub path: String,
    pub detail: String,
}

/// Follow schema-bearing keywords only: an example/const/default may itself
/// contain JSON properties named `$ref` without declaring a schema reference.
pub fn schema(root: &Value, draft: Draft) -> Result<(), Issue> {
    schema_many(&[(root, String::new())], draft)
}

/// References may promote an otherwise inert payload/const into a live schema.
/// All discovered targets share these limits; shared targets count only once.
pub fn schema_many(roots: &[(&Value, String)], draft: Draft) -> Result<(), Issue> {
    let mut pending = roots.to_vec();
    let mut seen = HashSet::new();
    let (mut count, mut patterns, mut branches) = (0, 0, 0);
    while let Some((value, path)) = pending.pop() {
        if !seen.insert(value as *const Value as usize) {
            continue;
        }
        count += 1;
        if count > MAX_SCHEMA_NODES {
            return Err(Issue {
                code: "schema.complexity",
                path,
                detail: MAX_SCHEMA_NODES.to_string(),
            });
        }
        let Some(map) = value.as_object() else {
            continue;
        };
        if let Some(uri) = map.get("$schema").and_then(Value::as_str)
            && !draft.matches_uri(uri)
        {
            return Err(Issue {
                code: "schema.draft_mismatch",
                path: pointer(&path, "$schema"),
                detail: uri.to_owned(),
            });
        }
        for keyword in ["$ref", "$dynamicRef", "$recursiveRef"] {
            if let Some(reference) = map.get(keyword).and_then(Value::as_str)
                && !reference.starts_with('#')
            {
                return Err(Issue {
                    code: "schema.external_ref",
                    path: pointer(&path, keyword),
                    detail: reference.chars().take(256).collect(),
                });
            }
        }
        if let Some(pattern) = map.get("pattern").and_then(Value::as_str) {
            patterns += 1;
            check_pattern(pattern, &pointer(&path, "pattern"))?;
        }
        if let Some(entries) = map.get("patternProperties").and_then(Value::as_object) {
            for pattern in entries.keys() {
                patterns += 1;
                check_pattern(
                    pattern,
                    &pointer(&pointer(&path, "patternProperties"), pattern),
                )?;
            }
        }
        if patterns > MAX_PATTERNS {
            return Err(Issue {
                code: "schema.pattern_budget",
                path,
                detail: MAX_PATTERNS.to_string(),
            });
        }
        for keyword in [
            "properties",
            "patternProperties",
            "$defs",
            "definitions",
            "dependentSchemas",
        ] {
            if let Some(entries) = map.get(keyword).and_then(Value::as_object) {
                let prefix = pointer(&path, keyword);
                pending.extend(
                    entries
                        .iter()
                        .map(|(key, value)| (value, pointer(&prefix, key))),
                );
            }
        }
        if let Some(entries) = map.get("dependencies").and_then(Value::as_object) {
            let prefix = pointer(&path, "dependencies");
            pending.extend(
                entries
                    .iter()
                    .filter(|(_, value)| !value.is_array())
                    .map(|(key, value)| (value, pointer(&prefix, key))),
            );
        }
        for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
            if let Some(entries) = map.get(keyword).and_then(Value::as_array) {
                if keyword != "prefixItems" {
                    branches += entries.len();
                }
                if branches > MAX_BRANCHES {
                    return Err(Issue {
                        code: "schema.branch_budget",
                        path: pointer(&path, keyword),
                        detail: MAX_BRANCHES.to_string(),
                    });
                }
                let prefix = pointer(&path, keyword);
                pending.extend(
                    entries
                        .iter()
                        .enumerate()
                        .map(|(i, value)| (value, pointer(&prefix, &i.to_string()))),
                );
            }
        }
        for keyword in [
            "additionalProperties",
            "additionalItems",
            "unevaluatedProperties",
            "unevaluatedItems",
            "propertyNames",
            "contains",
            "not",
            "if",
            "then",
            "else",
            "contentSchema",
            "items",
        ] {
            if let Some(value) = map.get(keyword) {
                let prefix = pointer(&path, keyword);
                if keyword == "items" && value.is_array() {
                    pending.extend(
                        value
                            .as_array()
                            .expect("checked array")
                            .iter()
                            .enumerate()
                            .map(|(i, value)| (value, pointer(&prefix, &i.to_string()))),
                    );
                } else {
                    pending.push((value, prefix));
                }
            }
        }
    }
    Ok(())
}

fn check_pattern(pattern: &str, path: &str) -> Result<(), Issue> {
    if pattern.len() > MAX_PATTERN_BYTES {
        Err(Issue {
            code: "schema.pattern_length",
            path: path.to_owned(),
            detail: MAX_PATTERN_BYTES.to_string(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "guard_test.rs"]
mod tests;
