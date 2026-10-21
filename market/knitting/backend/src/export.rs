use crate::model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportArgs {
    state: Workshop,
    format: String,
    path: Option<String>,
    language: String,
    #[serde(default)]
    part_only: bool,
}

fn escape(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '#' | '|' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

fn markdown(state: &Workshop, language: &str, part_only: bool) -> PluginResult<String> {
    let labels: Value = serde_json::from_str(if language.starts_with("zh") {
        include_str!("../../locales/zh-CN.json")
    } else {
        include_str!("../../locales/en-US.json")
    })
    .map_err(|_| error("file"))?;
    let text = |key: &str| labels["document"][key].as_str().unwrap_or(key).to_owned();
    let mut output = format!("# {}\n\n{}\n\n", text("title"), text("boundary"));
    for project in &state.projects {
        if part_only && Some(&project.id) != state.active_project_id.as_ref() {
            continue;
        }
        output.push_str(&format!("## {}\n\n", escape(&project.name)));
        for part in &project.parts {
            if part_only && Some(&part.id) != project.active_part_id.as_ref() {
                continue;
            }
            let unit = text(match part.kind {
                Kind::Rows => "rows",
                Kind::Rounds => "rounds",
            });
            output.push_str(&format!(
                "### {}\n\n- {}: {} {}\n- {}: {}\n- {}: {}\n- {}: {}\n\n",
                escape(&part.name),
                text("completed"),
                part.completed,
                unit,
                text("next"),
                part.completed + 1,
                text("stitches"),
                part.stitches,
                text("initial"),
                part.initial_stitches
            ));
            output.push_str(&format!("{}\n\n{}\n\n", text("note"), escape(&part.note)));
            output.push_str(&format!("#### {}\n\n", text("segments")));
            for (index, s) in part.segments.iter().enumerate() {
                let name = if s.name.is_empty() {
                    format!("{} {}", text("segment"), index + 1)
                } else {
                    escape(&s.name)
                };
                output.push_str(&format!(
                    "- {name}: {} {}–{}; {} {} × {}\n",
                    text("range"),
                    s.start,
                    s.end(),
                    text("repeat"),
                    s.rows,
                    s.repeats
                ));
            }
            output.push_str(&format!("\n#### {}\n\n", text("reminders")));
            for (index, r) in part.reminders.iter().enumerate() {
                let name = if r.name.is_empty() {
                    format!("{} {}", text("reminder"), index + 1)
                } else {
                    escape(&r.name)
                };
                output.push_str(&format!(
                    "- {name}: {} {}; {} {}; {} {:+}; {} {}\n",
                    text("start"),
                    r.start,
                    text("interval"),
                    r.interval,
                    text("delta"),
                    r.delta,
                    text("times"),
                    r.repeats
                ));
            }
            output.push('\n');
        }
    }
    Ok(output)
}

pub fn save(args: ExportArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    ctx.stage("validate");
    validate(&args.state, ctx)?;
    ctx.stage("export");
    let content = match args.format.as_str() {
        "json" => serde_json::to_string_pretty(&args.state).map_err(|_| error("file"))?,
        "markdown" => markdown(&args.state, &args.language, args.part_only)?,
        _ => return Err(error("format")),
    };
    if content.len() > MAX_EXPORT {
        return Err(error("export_limit"));
    }
    check_cancel(ctx)?;
    if let Some(path) = args.path {
        if path.trim().is_empty() {
            return Err(error("file"));
        }
        tf_plugin_guest::net::write_file(&path, content.as_bytes())?;
        Ok(json!({"saved":true}))
    } else {
        if args.format != "markdown" || !args.part_only || content.len() > 65536 {
            return Err(error("file"));
        }
        Ok(json!({"content":content}))
    }
}
