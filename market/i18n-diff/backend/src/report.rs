//! 差异报告：只列出有问题的单元格，CSV 每个问题一行，JSON 带上汇总。

use serde::Deserialize;
use serde_json::json;
use tf_plugin_api::{PluginError, PluginResult};

use crate::compare::{self, Issue};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    Csv,
    Json,
}

#[derive(Debug, Deserialize)]
pub struct Args {
    #[serde(flatten)]
    pub compare: compare::Args,
    pub format: ReportFormat,
}

fn issue_name(issue: Issue) -> &'static str {
    match issue {
        Issue::Missing => "missing",
        Issue::Extra => "extra",
        Issue::Empty => "empty",
        Issue::Untranslated => "untranslated",
        Issue::Placeholders => "placeholders",
        Issue::Duplicate => "duplicate",
    }
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

pub fn run(args: Args) -> PluginResult<String> {
    let output = compare::run(args.compare)?;
    match args.format {
        ReportFormat::Json => {
            let issues: Vec<_> = output
                .rows
                .iter()
                .flat_map(|row| {
                    row.cells
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| !c.issues.is_empty())
                        .map(|(i, cell)| {
                            json!({
                                "file": output.files[i].name,
                                "key": row.key,
                                "issues": cell.issues,
                                "base": row.cells[0].value,
                                "value": cell.value,
                                "missingPlaceholders": cell.missing_placeholders,
                                "extraPlaceholders": cell.extra_placeholders,
                            })
                        })
                })
                .collect();
            serde_json::to_string_pretty(&json!({ "files": output.files, "issues": issues }))
                .map_err(|e| PluginError::new("i18n.write_failed").with("detail", e.to_string()))
        }
        ReportFormat::Csv => {
            let mut text =
                String::from("file,key,issue,base,value,missing_placeholders,extra_placeholders\n");
            for row in &output.rows {
                for (i, cell) in row.cells.iter().enumerate() {
                    for issue in &cell.issues {
                        let fields = [
                            output.files[i].name.clone(),
                            row.key.clone(),
                            issue_name(*issue).to_owned(),
                            row.cells[0].value.clone().unwrap_or_default(),
                            cell.value.clone().unwrap_or_default(),
                            cell.missing_placeholders.join(" "),
                            cell.extra_placeholders.join(" "),
                        ];
                        let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
                        text.push_str(&line.join(","));
                        text.push('\n');
                    }
                }
            }
            Ok(text)
        }
    }
}
