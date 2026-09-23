use std::time::Instant;

use serde::{Deserialize, Serialize};
use sqlformat::{Dialect, FormatOptions, Indent, QueryParams};
use tf_plugin_api::{PluginError, PluginResult};

use crate::scan;

const MAX_INPUT: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub enum IndentKind {
    #[default]
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "4")]
    Four,
    #[serde(rename = "tab")]
    Tab,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeywordCase {
    #[default]
    Upper,
    Lower,
    Preserve,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialectKind {
    #[default]
    Generic,
    Postgresql,
    Sqlserver,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatArgs {
    input: String,
    #[serde(default)]
    indent: IndentKind,
    #[serde(default)]
    keyword_case: KeywordCase,
    #[serde(default)]
    dialect: DialectKind,
    #[serde(default = "one")]
    lines_between_queries: u8,
    /// 较短的参数列表与子句保持在同一行
    #[serde(default)]
    compact: bool,
}

fn one() -> u8 {
    1
}

#[derive(Deserialize)]
pub struct MinifyArgs {
    input: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub statements: usize,
    pub lines: usize,
    pub chars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    pub stats: Stats,
    pub elapsed_ms: f64,
}

fn check(input: &str) -> PluginResult<()> {
    if input.trim().is_empty() {
        return Err(PluginError::new("sql.empty"));
    }
    if input.len() > MAX_INPUT {
        return Err(PluginError::new("sql.too_large").with("limit", "5 MB"));
    }
    Ok(())
}

fn output(input: &str, output: String, start: Instant) -> Output {
    Output {
        stats: Stats {
            statements: scan::count_statements(input),
            lines: output.lines().count().max(1),
            chars: output.chars().count(),
        },
        output,
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    }
}

pub fn format(args: FormatArgs) -> PluginResult<Output> {
    check(&args.input)?;
    let start = Instant::now();
    let options = FormatOptions {
        indent: match args.indent {
            IndentKind::Two => Indent::Spaces(2),
            IndentKind::Four => Indent::Spaces(4),
            IndentKind::Tab => Indent::Tabs,
        },
        uppercase: match args.keyword_case {
            KeywordCase::Upper => Some(true),
            KeywordCase::Lower => Some(false),
            KeywordCase::Preserve => None,
        },
        lines_between_queries: args.lines_between_queries.clamp(1, 3),
        max_inline_arguments: args.compact.then_some(60),
        max_inline_top_level: args.compact.then_some(60),
        dialect: match args.dialect {
            DialectKind::Generic => Dialect::Generic,
            DialectKind::Postgresql => Dialect::PostgreSql,
            DialectKind::Sqlserver => Dialect::SQLServer,
        },
        ..FormatOptions::default()
    };
    let formatted = sqlformat::format(&args.input, &QueryParams::None, &options);
    Ok(output(&args.input, formatted, start))
}

pub fn minify(args: MinifyArgs) -> PluginResult<Output> {
    check(&args.input)?;
    let start = Instant::now();
    let minified = scan::minify(&args.input);
    Ok(output(&args.input, minified, start))
}

#[cfg(test)]
#[path = "format_test.rs"]
mod tests;
