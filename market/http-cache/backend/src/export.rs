use crate::model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Args {
    pub handle: String,
    pub format: String,
    pub language: String,
    pub path: Option<String>,
}
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '|' | '#' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}
fn label(labels: &Value, group: &str, key: &str) -> String {
    labels[group][key].as_str().unwrap_or(key).to_owned()
}
fn translated(labels: &Value, note: &Note) -> String {
    let mut text = label(labels, "messages", &note.code);
    if let Some(params) = note.params.as_object() {
        for (key, value) in params {
            let value = if key == "context" {
                label(labels, "context", value.as_str().unwrap_or_default())
            } else {
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
            };
            text = text.replace(&format!("{{{{{key}}}}}"), &value);
        }
    }
    text
}
fn markdown(reports: &[CaseReport], labels: &Value, ctx: &dyn TaskContext) -> PluginResult<String> {
    let text = |key: &str| label(labels, "document", key);
    let number = |n: Option<u64>| n.map_or_else(|| text("unknown"), |v| v.to_string());
    let mut out = format!("# {}\n\n{}\n\n", text("title"), text("boundary"));
    for report in reports {
        check(ctx)?;
        let s = &report.summary;
        out.push_str(&format!(
            "## {}\n\n{} / {}\n\n",
            escape(&s.name),
            escape(&report.method),
            escape(&report.status)
        ));
        for (key, value) in [
            (
                "role",
                label(
                    labels,
                    "role",
                    match s.role {
                        Role::Private => "private",
                        Role::Shared => "shared",
                    },
                ),
            ),
            ("storage", label(labels, "storage", &s.storage)),
            ("reuse", label(labels, "reuse", &s.reuse)),
            ("freshness", label(labels, "freshness", &s.freshness)),
            ("lifetime", number(s.lifetime)),
            ("age", number(s.current_age)),
        ] {
            out.push_str(&format!("- {}: {}\n", text(key), value));
        }
        out.push_str(&format!("\n### {}\n\n", text("calculation")));
        for (key, value) in [
            ("apparent_age", report.timing.apparent_age),
            ("response_delay", report.timing.response_delay),
            ("corrected_age_value", report.timing.corrected_age_value),
            ("corrected_initial_age", report.timing.corrected_initial_age),
            ("resident_time", report.timing.resident_time),
            ("current_age", report.timing.current_age),
        ] {
            out.push_str(&format!("- {key}: {}\n", number(value)));
        }
        out.push_str(&format!("\n### {}\n\n", text("conditions")));
        for c in &report.conditions {
            out.push_str(&format!(
                "- {}: {}\n",
                label(labels, "conditionStatus", &c.status),
                escape(&translated(labels, &c.note))
            ));
        }
        out.push_str(&format!("\n### {}\n\n", text("vary")));
        for row in &report.vary {
            let value = |value: &Option<String>| {
                escape(
                    value
                        .as_deref()
                        .unwrap_or_else(|| labels["vary"]["absent"].as_str().unwrap_or("absent")),
                )
            };
            out.push_str(&format!(
                "- {}: {} → {} ({})\n",
                escape(&row.name),
                value(&row.stored),
                value(&row.presented),
                label(labels, "vary", &row.status)
            ));
        }
        out.push_str(&format!("\n### {}\n\n", text("diagnostics")));
        for note in &report.diagnostics {
            out.push_str(&format!("- {}\n", escape(&translated(labels, note))));
        }
        out.push_str(&format!("\n### {}\n\n", text("conditional")));
        for line in report.conditional.lines() {
            out.push_str(&format!("    {line}\n"));
        }
        for (title, headers) in [
            ("request", &report.request_headers),
            ("response", &report.response_headers),
            ("presented", &report.new_request_headers),
        ] {
            out.push_str(&format!("\n### {}\n\n", text(title)));
            for header in headers {
                out.push_str(&format!("    {}: {}\n", header.name, header.value));
            }
        }
        out.push('\n');
    }
    Ok(out)
}
pub fn run(reports: &[CaseReport], args: Args, ctx: &dyn TaskContext) -> PluginResult<Value> {
    ctx.stage("export");
    ctx.log(
        tf_plugin_api::LogLevel::Info,
        "export_started",
        json!({"count":reports.len()}),
    );
    check(ctx)?;
    let labels: Value = serde_json::from_str(if args.language.starts_with("zh") {
        include_str!("../../locales/zh-CN.json")
    } else {
        include_str!("../../locales/en-US.json")
    })
    .map_err(|_| error("format"))?;
    let content = match args.format.as_str() {
        "json" => serde_json::to_string_pretty(&json!({
            "schemaVersion":1,"redacted":true,"scope":label(&labels,"document","boundary"),"cases":reports,
            "explanations":reports.iter().map(|r|json!({"name":r.summary.name,
                "conditions":r.conditions.iter().map(|c|translated(&labels,&c.note)).collect::<Vec<_>>(),
                "diagnostics":r.diagnostics.iter().map(|n|translated(&labels,n)).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        })).map_err(|_|error("format"))?,
        "markdown" => markdown(reports,&labels,ctx)?,
        _ => return Err(error("format")),
    };
    if content.len() > 2 * 1024 * 1024 {
        return Err(error("export_limit"));
    }
    check(ctx)?;
    if let Some(path) = args.path {
        if path.trim().is_empty() || path.len() > 4096 {
            return Err(error("path"));
        }
        tf_plugin_guest::net::write_file(&path, content.as_bytes())?;
        Ok(json!({"saved":true}))
    } else {
        if content.len() > 65536 {
            return Err(error("copy_limit"));
        }
        Ok(json!({"content":content}))
    }
}
