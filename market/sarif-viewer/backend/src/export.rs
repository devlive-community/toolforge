use crate::{
    model::*,
    view::{self, Filter},
};
use serde_json::json;
use tf_plugin_api::{PluginResult, TaskContext};

fn cell(text: &str) -> String {
    let text = if text
        .trim_start()
        .starts_with(['=', '+', '-', '@', '\t', '\r'])
    {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    format!("\"{}\"", text.replace('"', "\"\""))
}
fn md(text: &str) -> String {
    text.replace('&', "&amp;")
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
    report: &Report,
    filter: &Filter,
    format: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<String> {
    let rows = view::results(report, filter, ctx)?;
    let mut counts = Counts::default();
    for row in &rows {
        counts.add_result(row);
    }
    let output = match format {
        "json" => {
            let items: Vec<_> = rows
                .iter()
                .map(|row| {
                    let primary = report.primary_location(row);
                    json!({
                        "result":row,
                        "uri":primary.and_then(|l| l.uri.clone()),
                        "startLine":primary.and_then(|l| l.region).and_then(|r| r.start_line),
                        "startColumn":primary.and_then(|l| l.region).and_then(|r| r.start_column),
                        "snippet":primary.and_then(|l| l.snippet.clone()),
                        "tool":report.runs[row.run].tool,
                        "suppressionCount":row.suppressions.len(),
                    })
                })
                .collect();
            serde_json::to_string_pretty(&json!({
                "formatVersion":1,
                "counts":counts,
                "runs":report.runs,
                "results":items,
                "diagnostics":report.diagnostics,
                "codeFlowsIncluded":false,
            }))
            .map_err(|_| error("export"))?
        }
        "csv" => {
            let mut output = "result_id,run,tool,rule_id,level,kind,suppressed,baseline_state,artifact_uri,start_line,start_column,message\r\n".to_owned();
            for row in rows {
                check(ctx)?;
                let level = serde_json::to_value(row.level).map_err(|_| error("export"))?;
                let primary = report.primary_location(row);
                let values = [
                    row.id.to_string(),
                    row.run.to_string(),
                    report.runs[row.run].tool.clone(),
                    row.rule_id.clone(),
                    level.as_str().unwrap_or("").into(),
                    row.kind.clone(),
                    if row.suppressed { "yes" } else { "no" }.into(),
                    row.baseline_state.clone().unwrap_or_default(),
                    primary.and_then(|l| l.uri.clone()).unwrap_or_default(),
                    primary
                        .and_then(|l| l.region)
                        .and_then(|r| r.start_line)
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    primary
                        .and_then(|l| l.region)
                        .and_then(|r| r.start_column)
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    row.message.clone(),
                ];
                output.push_str(&values.iter().map(|s| cell(s)).collect::<Vec<_>>().join(","));
                output.push_str("\r\n");
                if output.len() > 64 * 1024 * 1024 {
                    return Err(error("output"));
                }
            }
            output
        }
        "markdown" => {
            let mut output = format!(
                "# SARIF report\n\nResults: {} · Errors: {} · Warnings: {} · Notes: {} · None: {} · Suppressed: {}\n\nFull code flows and related-location snippets are excluded.\n\n| ID | Run | Tool | Rule | Level | Kind | Location | Message |\n|---|---|---|---|---|---|---|---|\n",
                counts.results,
                counts.errors,
                counts.warnings,
                counts.notes,
                counts.none,
                counts.suppressed,
            );
            for row in rows {
                check(ctx)?;
                let level = serde_json::to_value(row.level).map_err(|_| error("export"))?;
                let primary = report.primary_location(row);
                let location = match primary.and_then(|l| l.uri.clone()) {
                    Some(uri) => {
                        let line = primary
                            .and_then(|l| l.region)
                            .and_then(|r| r.start_line)
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        format!("{md_uri}:{line}", md_uri = md(&uri), line = line)
                    }
                    None => primary
                        .and_then(|l| l.logical.clone())
                        .map(|logical| md(&logical))
                        .unwrap_or_else(|| "—".into()),
                };
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                    row.id,
                    row.run,
                    md(&report.runs[row.run].tool),
                    md(&row.rule_id),
                    level.as_str().unwrap_or(""),
                    md(&row.kind),
                    location,
                    md(&row.message),
                ));
                if output.len() > 64 * 1024 * 1024 {
                    return Err(error("output"));
                }
            }
            output
        }
        _ => return Err(error("format")),
    };
    check(ctx)?;
    if output.len() > 64 * 1024 * 1024 {
        return Err(error("output"));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "export_test.rs"]
mod tests;
