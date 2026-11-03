use crate::model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filter {
    pub run: Option<usize>,
    pub level: Option<Level>,
    pub rule_id: Option<String>,
    pub artifact: Option<usize>,
    pub suppressed: Option<bool>,
    pub baseline: Option<String>,
    #[serde(default)]
    pub query: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseArgs {
    pub handle: String,
    pub kind: String,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "page_size")]
    pub limit: usize,
}
fn page_size() -> usize {
    PAGE
}
pub fn validate_page(offset: usize, limit: usize) -> PluginResult<()> {
    if limit == 0 || limit > 100 || offset > ENTRY_LIMIT {
        Err(error("page"))
    } else {
        Ok(())
    }
}

/// Results matching the current filter; `primary_location` drives artifact
/// filtering while the query scans message, ruleId and location URIs.
pub fn results<'a>(
    report: &'a Report,
    filter: &Filter,
    ctx: &dyn TaskContext,
) -> PluginResult<Vec<&'a ResultRow>> {
    if filter.run.is_some_and(|id| id >= report.runs.len())
        || filter
            .artifact
            .is_some_and(|id| id >= report.artifacts.len())
        || filter
            .rule_id
            .as_ref()
            .is_some_and(|id| id.len() > STRING_LIMIT)
        || filter
            .baseline
            .as_ref()
            .is_some_and(|id| id.len() > STRING_LIMIT)
        || filter.query.len() > 512
    {
        return Err(error("page"));
    }
    let needle = filter.query.trim().to_lowercase();
    let mut rows = Vec::new();
    for row in &report.results {
        check(ctx)?;
        let primary = report.primary_location(row).map(|location| {
            location
                .uri
                .clone()
                .or_else(|| location.logical.clone())
                .unwrap_or_default()
        });
        let artifact = report
            .primary_location(row)
            .and_then(|location| location.artifact);
        if filter.run.is_none_or(|id| id == row.run)
            && filter.level.is_none_or(|level| level == row.level)
            && filter.rule_id.as_ref().is_none_or(|id| *id == row.rule_id)
            && filter.artifact.is_none_or(|id| artifact == Some(id))
            && filter
                .suppressed
                .is_none_or(|value| value == row.suppressed)
            && filter
                .baseline
                .as_deref()
                .is_none_or(|value| row.baseline_state.as_deref() == Some(value))
            && (needle.is_empty()
                || row.message.to_lowercase().contains(&needle)
                || row.rule_id.to_lowercase().contains(&needle)
                || primary.is_some_and(|uri| uri.to_lowercase().contains(&needle)))
        {
            rows.push(row);
        }
    }
    check(ctx)?;
    Ok(rows)
}

pub fn browse(report: &Report, args: &BrowseArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    validate_page(args.offset, args.limit)?;
    let (offset, limit) = (args.offset, args.limit);
    match args.kind.as_str() {
        "runs" => {
            if args.filter.run.is_some_and(|id| id >= report.runs.len()) {
                return Err(error("page"));
            }
            let rows: Vec<_> = report
                .runs
                .iter()
                .filter(|row| args.filter.run.is_none_or(|id| id == row.id))
                .collect();
            Ok(json!({"total":rows.len(),"items":rows}))
        }
        "rules" => {
            if args.filter.run.is_some_and(|id| id >= report.runs.len()) {
                return Err(error("page"));
            }
            let rows: Vec<_> = report
                .rules
                .iter()
                .filter(|row| args.filter.run.is_none_or(|id| id == row.run))
                .collect();
            Ok(
                json!({"total":rows.len(),"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        "artifacts" => {
            if args.filter.run.is_some_and(|id| id >= report.runs.len()) {
                return Err(error("page"));
            }
            let rows: Vec<_> = report
                .artifacts
                .iter()
                .filter(|row| {
                    args.filter.run.is_none_or(|id| id == row.run)
                        && (needle(&args.filter).is_empty()
                            || row.uri.to_lowercase().contains(&needle(&args.filter)))
                })
                .collect();
            Ok(
                json!({"total":rows.len(),"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        "results" => {
            let rows = results(report, &args.filter, ctx)?;
            let mut counts = Counts::default();
            for row in &rows {
                counts.add_result(row);
            }
            Ok(
                json!({"total":rows.len(),"counts":counts,"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        "diagnostics" => {
            if args.filter.run.is_some_and(|id| id >= report.runs.len()) {
                return Err(error("page"));
            }
            let rows: Vec<_> = report
                .diagnostics
                .iter()
                .filter(|row| args.filter.run.is_none_or(|id| id == row.run))
                .collect();
            Ok(
                json!({"total":rows.len(),"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        _ => Err(error("page")),
    }
}

fn needle(filter: &Filter) -> String {
    filter.query.trim().to_lowercase()
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
