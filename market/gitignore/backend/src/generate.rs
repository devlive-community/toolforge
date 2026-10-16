use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::{
    catalog,
    library::{self, Library},
};

pub const MAX_USER_BYTES: usize = 1_048_576;
const MAX_SELECTIONS: usize = 128;

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    #[serde(default)]
    pub template_ids: Vec<String>,
    #[serde(default)]
    pub base_content: String,
    #[serde(default)]
    pub custom_rules: String,
    #[serde(default)]
    pub line_ending: LineEnding,
    #[serde(default)]
    pub library: Library,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    template_count: usize,
    rule_count: usize,
    duplicate_count: usize,
    line_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    id: String,
    name: String,
    rule_count: usize,
}

#[derive(Serialize)]
pub struct Output {
    pub output: String,
    pub stats: Stats,
    pub sections: Vec<Section>,
}

/// Git ignores unescaped ASCII spaces at the end, but leading spaces, tabs,
/// escaped spaces, backslashes and comment-like patterns remain significant.
pub fn is_rule(line: &str) -> bool {
    if line.starts_with('#') {
        return false;
    }
    let effective = line.trim_end_matches(' ');
    !effective.is_empty() && effective != "!"
}

/// Git removes a leading BOM and one trailing CR per physical input line,
/// including a final line without LF. Interior or additional CRs are patterns.
pub fn source_lines(text: &str) -> impl Iterator<Item = &str> {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .split_terminator('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

struct Merger<'a> {
    lines: Vec<&'a str>,
    header_indices: Vec<usize>,
    seen: HashSet<&'a str>,
    stats: Stats,
    sections: Vec<Section>,
}

impl<'a> Merger<'a> {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            header_indices: Vec::new(),
            seen: HashSet::new(),
            stats: Stats::default(),
            sections: Vec::new(),
        }
    }

    fn add(&mut self, id: &str, name: &str, text: &'a str) {
        // source_lines removes exactly one leading BOM. Do not strip it here
        // too: a second BOM is a literal part of the first pattern in Git.
        if text.strip_prefix('\u{feff}').unwrap_or(text).is_empty() {
            return;
        }
        let mut rule_count = 0;
        self.lines.push("");
        let header_index = self.lines.len();
        self.lines.push("");
        for line in source_lines(text) {
            // All positive rules in one segment have the same ignore outcome.
            // A negation can reverse it: never drop a negation, and retain the
            // next occurrence of every positive rule after any negation.
            let negation = line.starts_with('!');
            if negation {
                self.seen.clear();
            }
            if is_rule(line) {
                if !negation && !self.seen.insert(line) {
                    self.stats.duplicate_count += 1;
                    continue;
                }
                rule_count += 1;
            }
            self.lines.push(line);
        }
        // Headers are inserted when rendering so their owned names never need
        // to borrow from input buffers.
        self.sections.push(Section {
            id: id.to_owned(),
            name: name.to_owned(),
            rule_count,
        });
        self.stats.rule_count += rule_count;
        self.header_indices.push(header_index);
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    library::validate(&args.library)?;
    if args.template_ids.len() > MAX_SELECTIONS {
        return Err(PluginError::new("gitignore.tooManyTemplates").with("max", MAX_SELECTIONS));
    }
    if args
        .base_content
        .len()
        .saturating_add(args.custom_rules.len())
        > MAX_USER_BYTES
    {
        return Err(PluginError::new("gitignore.tooLarge").with("max", MAX_USER_BYTES));
    }
    for (field, text) in [
        ("baseContent", &args.base_content),
        ("customRules", &args.custom_rules),
    ] {
        if text.contains('\0') {
            return Err(PluginError::new("gitignore.invalidText").with("field", field));
        }
    }
    let mut selected = HashSet::new();
    let mut templates = Vec::new();
    for id in &args.template_ids {
        let template = if let Some(template) = catalog::find(id) {
            (template.id, template.name, template.text)
        } else if let Some(template) = args
            .library
            .templates
            .iter()
            .find(|template| template.id == *id)
        {
            (
                template.id.as_str(),
                template.name.as_str(),
                template.content.as_str(),
            )
        } else {
            return Err(PluginError::new("gitignore.unknownTemplate").with("id", id.clone()));
        };
        if selected.insert(id) {
            templates.push(template);
        }
    }
    let mut merger = Merger::new();
    merger.add("base", "Base .gitignore", &args.base_content);
    for template in &templates {
        merger.add(template.0, template.1, template.2);
    }
    merger.add("custom", "Custom rules", &args.custom_rules);
    merger.stats.template_count = templates.len();

    let mut output = String::new();
    if !merger.sections.is_empty() {
        output.push_str("# Generated by ToolForge .gitignore Generator\n");
        let mut sections = merger.sections.iter();
        let mut next_header = merger.header_indices.iter();
        let mut header_index = next_header.next();
        for (index, line) in merger.lines.iter().enumerate() {
            if header_index == Some(&index) {
                let section = sections.next().expect("one header per section");
                output.push_str(&format!("# --- {} ---", section.name));
                header_index = next_header.next();
            } else {
                output.push_str(line);
                // An additional literal trailing CR must not become CRLF on
                // rendering. Git discards this unescaped space after its CRLF
                // handling, preserving the intended literal CR pattern.
                if line.ends_with('\r') {
                    output.push(' ');
                }
            }
            output.push('\n');
        }
    }
    merger.stats.line_count = output.lines().count();
    if matches!(args.line_ending, LineEnding::Crlf) {
        output = output.replace('\n', "\r\n");
    }
    Ok(Output {
        output,
        stats: merger.stats,
        sections: merger.sections,
    })
}

#[cfg(test)]
#[path = "generate_test.rs"]
mod tests;

#[cfg(all(test, not(target_family = "wasm")))]
#[path = "git_test.rs"]
mod git_tests;
