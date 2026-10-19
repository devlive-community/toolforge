//! 报告导出：JSON（结构化）/ Markdown / 纯文本。导出文本为英文技术报告
//! （不本地化），界面展示走 i18n。报告注明规则版本与未检查范围。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::diff::{Direction, Finding, Report, Severity};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub format: String,
    pub report: Report,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub content: String,
    pub ext: String,
    pub markdown: String,
}

pub fn export(args: ExportArgs) -> PluginResult<Exported> {
    match args.format.as_str() {
        "json" => {
            let content = serde_json::to_string_pretty(&args.report)
                .map_err(|_| PluginError::new("pbd.serialize"))?;
            Ok(Exported {
                markdown: markdown(&args.report),
                content,
                ext: "json".to_owned(),
            })
        }
        "markdown" => {
            let content = markdown(&args.report);
            Ok(Exported {
                markdown: content.clone(),
                content,
                ext: "md".to_owned(),
            })
        }
        "text" => Ok(Exported {
            markdown: markdown(&args.report),
            content: text(&args.report),
            ext: "txt".to_owned(),
        }),
        other => Err(PluginError::new("pbd.unknown_format").with("format", other)),
    }
}

fn severity_text(severity: Severity) -> &'static str {
    match severity {
        Severity::Breaking => "breaking",
        Severity::Conditional => "conditional",
        Severity::Info => "info",
    }
}

fn direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::OldWriteNewRead => "old→new",
        Direction::NewWriteOldRead => "new→old",
        Direction::Both => "both",
        Direction::ProtoJson => "protojson",
    }
}

fn headline(report: &Report) -> String {
    format!(
        "Protobuf compatibility report (rules v{}) — {} breaking, {} conditional, {} info",
        report.rules_version,
        report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Breaking)
            .count(),
        report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Conditional)
            .count(),
        report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Info)
            .count(),
    )
}

fn text(report: &Report) -> String {
    let mut lines = vec![headline(report)];
    if !report.unsupported.is_empty() {
        lines.push(String::new());
        lines.push("Unsupported (excluded from conclusions):".to_owned());
        for file in &report.unsupported {
            lines.push(format!("  {} ({})", file.name, file.syntax));
        }
    }
    for finding in &report.findings {
        lines.push(format!(
            "[{}] {:?} {} {} | old: {} | new: {} | {}",
            severity_text(finding.severity),
            finding.direction,
            finding.rule,
            finding.path,
            finding.old_desc,
            finding.new_desc,
            finding.location,
        ));
    }
    lines.push(String::new());
    lines.push("Not checked:".to_owned());
    for note in &report.notes {
        lines.push(format!("  - {note}"));
    }
    lines.join("\n")
}

fn markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# Protobuf Compatibility Report\n\n");
    out.push_str(&format!("Rules version: {}\n\n", report.rules_version));
    if !report.unsupported.is_empty() {
        out.push_str("## Unsupported files (excluded from conclusions)\n\n");
        for file in &report.unsupported {
            out.push_str(&format!("- `{}` — syntax `{}`\n", file.name, file.syntax));
        }
        out.push('\n');
    }
    for (title, severity) in [
        ("Breaking", Severity::Breaking),
        ("Conditional", Severity::Conditional),
        ("Info", Severity::Info),
    ] {
        let items: Vec<&Finding> = report
            .findings
            .iter()
            .filter(|f| f.severity == severity)
            .collect();
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("## {title}\n\n"));
        for finding in items {
            let change = if finding.new_desc.is_empty() {
                finding.old_desc.clone()
            } else if finding.old_desc.is_empty() {
                finding.new_desc.clone()
            } else {
                format!("{} → {}", finding.old_desc, finding.new_desc)
            };
            out.push_str(&format!(
                "- **{}** `{}` ({:?}, {}): {}\n",
                finding.rule,
                finding.path,
                finding.direction,
                direction_text(finding.direction),
                change
            ));
            if !finding.location.is_empty() {
                out.push_str(&format!("  - location: `{}`\n", finding.location));
            }
        }
        out.push('\n');
    }
    out.push_str("## Not checked\n\n");
    for note in &report.notes {
        out.push_str(&format!("- {note}\n"));
    }
    out
}

#[cfg(test)]
#[path = "report_test.rs"]
mod tests;
