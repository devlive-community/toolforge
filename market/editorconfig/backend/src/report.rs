//! 导出报告：JSON（完整结果）或 Markdown（每个目标的最终属性与来源）。

use serde::Deserialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::resolve;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Markdown,
}

#[derive(Debug, Deserialize)]
pub struct Args {
    #[serde(flatten)]
    pub resolve: resolve::Args,
    pub format: Format,
}

fn file_label(dir: &str) -> String {
    if dir.is_empty() {
        ".editorconfig".to_owned()
    } else {
        format!("{dir}/.editorconfig")
    }
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

pub fn run(args: Args) -> PluginResult<String> {
    let output = resolve::run(args.resolve)?;
    match args.format {
        Format::Json => serde_json::to_string_pretty(&output)
            .map_err(|e| PluginError::new("ec.export_failed").with("detail", e.to_string())),
        Format::Markdown => {
            let mut out = format!("# EditorConfig\n\nSpecification {}\n", output.spec_version);
            for target in &output.targets {
                out.push_str(&format!("\n## `{}`\n\n", target.path));
                if target.incomplete {
                    out.push_str("> No `root = true` within the project; configuration above the project folder was not read.\n\n");
                }
                out.push_str("| Property | Value | Source |\n|---|---|---|\n");
                for prop in &target.properties {
                    let value = prop.value.as_deref().unwrap_or("(unset)");
                    let source = match &prop.source {
                        Some(s) => {
                            format!("{}:{} `[{}]`", file_label(&s.dir), s.line, cell(&s.section))
                        }
                        None if prop.kind == "derived" => "derived".to_owned(),
                        None => "unset".to_owned(),
                    };
                    let mut value = cell(value);
                    if prop.invalid {
                        value.push_str(" ⚠");
                    }
                    if prop.kind == "extension" {
                        value.push_str(" (extension)");
                    }
                    out.push_str(&format!("| `{}` | {} | {} |\n", prop.key, value, source));
                }
                if !target.unset_standard.is_empty() {
                    out.push_str(&format!(
                        "\nNot set: {}\n",
                        target
                            .unset_standard
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            if !output.problems.is_empty() {
                out.push_str("\n## Problems\n\n");
                for p in &output.problems {
                    out.push_str(&format!(
                        "- {}:{} {} `{}`\n",
                        file_label(&p.dir),
                        p.problem.line,
                        p.problem.kind,
                        p.problem.text
                    ));
                }
            }
            Ok(out)
        }
    }
}
