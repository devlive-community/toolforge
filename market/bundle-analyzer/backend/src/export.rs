use crate::{
    analysis::{self, Query},
    model::*,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub query: Query,
    pub format: String,
    pub path: Option<String>,
}
fn csv(v: &Value) -> String {
    let s = v.as_str().map(str::to_owned).unwrap_or_else(|| {
        if v.is_null() {
            String::new()
        } else {
            v.to_string()
        }
    });
    let s = if v.is_string() && s.trim_start().starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s
    };
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn md(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if v.is_null() {
                "—".into()
            } else {
                v.to_string()
            }
        })
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}
pub fn render(
    reports: &Reports,
    q: &Query,
    format: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<String> {
    let rows = analysis::rows(reports, q, ctx)?;
    if format == "json" {
        return serde_json::to_string_pretty(&json!({"formatVersion":1,"scope":{"kind":q.kind,"side":q.side,"output":q.output,"other":q.other,"search":q.search,"directory":q.directory,"module":q.module,"pairs":q.pairs},"before":{"name":reports.before.name,"totals":reports.before.totals},"after":reports.after.as_ref().map(|s|json!({"name":s.name,"totals":s.totals})),"rows":rows,"units":"bytesInOutput; sourceBytes are original source bytes; gzip/Brotli sizes are not inferred"})).map_err(|_|error("export"));
    }
    if !["csv", "markdown"].contains(&format) {
        return Err(error("format"));
    }
    let columns = rows
        .iter()
        .filter_map(Value::as_object)
        .flat_map(|o| o.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut output = if format == "csv" {
        columns
            .iter()
            .map(|s| csv(&json!(s)))
            .collect::<Vec<_>>()
            .join(",")
            + "\r\n"
    } else {
        format!(
            "# Bundle attribution\n\nScope: {}\n\nContributions use bytesInOutput; sourceBytes are original source size. Missing attribution is not assigned to modules. No gzip/Brotli sizes are inferred.\n\n| {} |\n|{}|\n",
            md(&json!(q.kind)),
            columns.join(" | "),
            columns.iter().map(|_| "---").collect::<Vec<_>>().join("|")
        )
    };
    for row in rows {
        check(ctx)?;
        let cells = columns
            .iter()
            .map(|key| {
                if format == "csv" {
                    csv(&row[key])
                } else {
                    md(&row[key])
                }
            })
            .collect::<Vec<_>>();
        if format == "csv" {
            output.push_str(&cells.join(","));
            output.push_str("\r\n");
        } else {
            output.push_str(&format!("| {} |\n", cells.join(" | ")));
        }
        if output.len() > 64 * 1024 * 1024 {
            return Err(error("output_limit"));
        }
    }
    Ok(output)
}
pub fn run(
    reports: &Reports,
    args: Request,
    copy: bool,
    ctx: &dyn TaskContext,
) -> PluginResult<Value> {
    if !copy && let Some(path) = &args.path {
        let identity = |p: &str| p.replace('\\', "/").to_lowercase();
        if identity(path) == identity(&reports.before.source)
            || reports
                .after
                .as_ref()
                .is_some_and(|s| identity(path) == identity(&s.source))
        {
            return Err(error("source"));
        }
    }
    ctx.stage("export");
    let text = render(reports, &args.query, &args.format, ctx)?;
    check(ctx)?;
    if text.len() > 64 * 1024 * 1024 {
        return Err(error("output_limit"));
    }
    if copy {
        if text.len() > 65536 {
            return Err(error("copy_limit"));
        }
        return Ok(json!({"content":text}));
    }
    tf_plugin_guest::net::write_file(&args.path.ok_or_else(|| error("file"))?, text.as_bytes())?;
    Ok(json!({"saved":true}))
}
