use crate::model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadSourceArgs {
    pub handle: String,
    pub location: usize,
    pub root: String,
    #[serde(default = "context_lines")]
    pub context: usize,
}
fn context_lines() -> usize {
    SOURCE_CONTEXT_MAX.min(4)
}

/// Decode `%xx` escapes; invalid or non-UTF-8 output fails so encoded
/// traversal payloads cannot slip through as opaque strings.
pub fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16)?;
            let low = (bytes[index + 2] as char).to_digit(16)?;
            out.push((high * 16 + low) as u8);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Resolve an artifact URI against `originalUriBaseIds` and the authorized
/// source root. Every candidate later has to pass the canonical containment
/// check, so a relative base or an absolute `file://` URI both end up inside
/// the authorized root or are rejected.
pub fn resolve(
    report: &Report,
    location: &LocationRow,
    root: &str,
) -> PluginResult<(String, Vec<String>)> {
    let mut notes = Vec::new();
    let uri = location.uri.as_deref().unwrap_or("");
    if uri.contains('\0') {
        return Err(error("escape"));
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        let rest = percent_decode(rest).ok_or_else(|| error("escape"))?;
        return Ok((rest, notes));
    }
    if uri.contains('%') {
        notes.push("percent".into());
    }
    let relative = percent_decode(uri).ok_or_else(|| error("escape"))?;
    if relative.contains('\0') || relative.contains('\\') {
        return Err(error("escape"));
    }
    if relative.split('/').any(|segment| segment == "..") {
        return Err(error("escape"));
    }
    let base = location.uri_base_id.as_ref().and_then(|key| {
        report
            .bases
            .iter()
            .find(|base| base.run == location.run && &base.key == key)
    });
    let joined = match base {
        Some(base) if base.uri.starts_with("file://") => {
            let stem = base.uri.strip_prefix("file://").unwrap_or("");
            let stem = percent_decode(stem).ok_or_else(|| error("escape"))?;
            format!(
                "{}/{}",
                stem.trim_end_matches('/'),
                relative.trim_start_matches('/')
            )
        }
        Some(_) => {
            notes.push("baseNotLocal".into());
            join_root(root, &relative)
        }
        None => {
            if location.uri_base_id.is_some() {
                notes.push("baseUnknown".into());
            }
            join_root(root, &relative)
        }
    };
    Ok((joined, notes))
}

fn join_root(root: &str, relative: &str) -> String {
    if relative.starts_with('/') {
        relative.to_owned()
    } else {
        format!("{}/{}", root.trim_end_matches('/'), relative)
    }
}

/// Read the lines around a result's region from the authorized source root.
/// Without a region the tool refuses to guess a range.
pub fn read(report: &Report, args: &ReadSourceArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    if args.context > SOURCE_CONTEXT_MAX || args.root.is_empty() || args.root.len() > 4096 {
        return Err(error("page"));
    }
    let location = report
        .locations
        .get(args.location)
        .ok_or_else(|| error("page"))?;
    let uri = location.uri.clone().ok_or_else(|| error("noUri"))?;
    let region = location.region.ok_or_else(|| error("region"))?;
    let start_line = region.start_line.ok_or_else(|| error("region"))?;
    let (candidate, mut notes) = resolve(report, location, &args.root)?;
    let root_canon = std::fs::canonicalize(&args.root).map_err(|_| error("root"))?;
    // Lexical containment first: absolute paths outside (or unrelated to) the
    // authorized root are refused before touching the filesystem, which also
    // keeps the error stable across platforms for paths that do not exist.
    let relative = std::path::Path::new(&candidate)
        .strip_prefix(&args.root)
        .map_err(|_| error("outside"))?;
    let path = std::fs::canonicalize(root_canon.join(relative)).map_err(|_| error("missing"))?;
    // Canonical containment second: symlinks may not escape the root either.
    if !path.starts_with(&root_canon) {
        return Err(error("outside"));
    }
    check(ctx)?;
    let first = start_line.saturating_sub(args.context as u64).max(1);
    let end = region.end_line.unwrap_or(start_line);
    let last = end.saturating_add(args.context as u64);
    let mut reader = BufReader::new(ctx.open_file(&path.to_string_lossy())?);
    let mut lines = Vec::new();
    let mut truncated = false;
    let mut number = 0u64;
    let mut buffer = String::new();
    loop {
        check(ctx)?;
        buffer.clear();
        let size = reader
            .read_line(&mut buffer)
            .map_err(|_| error("encoding"))?;
        if size == 0 {
            break;
        }
        number += 1;
        if number < first {
            continue;
        }
        if lines.len() >= SOURCE_MAX_LINES || number > last {
            truncated = true;
            break;
        }
        let text = buffer.trim_end_matches(['\n', '\r']);
        let text: String = {
            let mut owned = text.chars().take(STRING_LIMIT).collect::<String>();
            if owned.chars().count() < text.chars().count() {
                truncated = true;
                notes.push("lineTruncated".into());
                owned.push('…');
            }
            owned
        };
        lines.push(json!({"no":number,"text":text}));
    }
    Ok(json!({
        "uri":uri,
        "resolved":path.to_string_lossy(),
        "columnKind":report.runs[location.run].column_kind,
        "region":region,
        "lines":lines,
        "truncated":truncated,
        "notes":notes,
    }))
}

#[cfg(test)]
#[path = "source_test.rs"]
mod tests;
