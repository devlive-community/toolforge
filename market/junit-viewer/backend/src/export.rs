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
    let rows = view::cases(report, filter, ctx)?;
    let mut counts = Counts::default();
    for case in &rows {
        counts.add_case(case);
    }
    let output = match format {
        "json" => {
            let cases: Vec<_> = rows.iter().map(|c| json!({"case":c,"findings":c.content.iter().map(|&id|view::content_meta(&report.content[id])).collect::<Vec<_>>()})).collect();
            serde_json::to_string_pretty(&json!({"formatVersion":1,"counts":counts,"sources":report.sources,"suites":report.suites,"cases":cases,"diagnostics":report.diagnostics,"logsIncluded":false})).map_err(|_| error("export"))?
        }
        "csv" => {
            let mut output =
                "source_id,source,suite_id,suite,case_id,classname,name,status,seconds\r\n"
                    .to_owned();
            for c in rows {
                check(ctx)?;
                let status = serde_json::to_value(c.status).map_err(|_| error("export"))?;
                let values = [
                    c.source.to_string(),
                    report.sources[c.source].path.clone(),
                    c.suite.to_string(),
                    report.suites[c.suite].name.clone(),
                    c.id.to_string(),
                    c.classname.clone(),
                    c.name.clone(),
                    status.as_str().unwrap_or("").into(),
                    c.seconds.map(|s| s.to_string()).unwrap_or_default(),
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
                "# JUnit XML\n\nTests: {} · Passed: {} · Failures: {} · Errors: {} · Skipped: {} · Unknown: {}\n\nKnown testcase seconds: {} · Missing/invalid time: {}\n\nFull stdout/stderr and failure bodies are excluded.\n\n| Source / suite / case ID | Class | Test | Status | Seconds |\n|---|---|---|---|---|\n",
                counts.tests,
                counts.passed,
                counts.failures,
                counts.errors,
                counts.skipped,
                counts.unknown,
                counts.known_seconds,
                counts.missing_time
            );
            for c in rows {
                check(ctx)?;
                let status = serde_json::to_value(c.status).map_err(|_| error("export"))?;
                output.push_str(&format!(
                    "| {} / {} / {} | {} | {} | {} | {} |\n",
                    c.source,
                    c.suite,
                    c.id,
                    md(&c.classname),
                    md(&c.name),
                    status.as_str().unwrap_or(""),
                    c.seconds
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "—".into())
                ));
                if output.len() > 64 * 1024 * 1024 {
                    return Err(error("output"));
                }
            }
            output.push_str("\n## Sources\n\n");
            for source in &report.sources {
                output.push_str(&format!("- {}: {}\n", source.id, md(&source.path)));
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
