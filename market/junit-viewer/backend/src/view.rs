use crate::model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::cmp::Ordering;
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filter {
    pub source: Option<usize>,
    pub suite: Option<usize>,
    pub status: Option<Status>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub slowest: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowseArgs {
    pub handle: String,
    pub kind: String,
    #[serde(default)]
    pub filter: Filter,
    pub parent: Option<usize>,
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
pub fn descendants(report: &Report, mut id: usize, ancestor: usize) -> bool {
    loop {
        if id == ancestor {
            return true;
        }
        match report.suites[id].parent {
            Some(parent) => id = parent,
            None => return false,
        }
    }
}
pub fn cases<'a>(
    report: &'a Report,
    filter: &Filter,
    ctx: &dyn TaskContext,
) -> PluginResult<Vec<&'a Case>> {
    if filter.source.is_some_and(|id| id >= report.sources.len())
        || filter.suite.is_some_and(|id| id >= report.suites.len())
        || filter.query.len() > 512
    {
        return Err(error("page"));
    }
    let needle = filter.query.trim().to_lowercase();
    let mut rows = Vec::new();
    for case in &report.cases {
        check(ctx)?;
        if filter.source.is_none_or(|id| id == case.source)
            && filter
                .suite
                .is_none_or(|id| descendants(report, case.suite, id))
            && filter.status.is_none_or(|s| s == case.status)
            && (needle.is_empty()
                || case.name.to_lowercase().contains(&needle)
                || case.classname.to_lowercase().contains(&needle))
        {
            rows.push(case);
        }
    }
    if filter.slowest {
        rows.sort_by(|a, b| {
            match (a.seconds, b.seconds) {
                (Some(a), Some(b)) => b.total_cmp(&a),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                _ => Ordering::Equal,
            }
            .then(a.id.cmp(&b.id))
        });
    }
    check(ctx)?;
    Ok(rows)
}
pub fn content_meta(c: &Content) -> Value {
    json!({"id":c.id,"kind":c.kind,"message":c.message,"typeName":c.type_name,"characters":c.text.chars().count()})
}
pub fn browse(report: &Report, args: &BrowseArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
    validate_page(args.offset, args.limit)?;
    let (offset, limit) = (args.offset, args.limit);
    match args.kind.as_str() {
        "sources" => Ok(
            json!({"total":report.sources.len(),"items":report.sources.iter().skip(offset).take(limit).collect::<Vec<_>>()}),
        ),
        "suites" => {
            if args.parent.is_some_and(|id| id >= report.suites.len())
                || args
                    .filter
                    .source
                    .is_some_and(|id| id >= report.sources.len())
            {
                return Err(error("page"));
            }
            let rows: Vec<_> = report
                .suites
                .iter()
                .filter(|s| {
                    s.parent == args.parent && args.filter.source.is_none_or(|id| s.source == id)
                })
                .collect();
            Ok(
                json!({"total":rows.len(),"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        "cases" => {
            let rows = cases(report, &args.filter, ctx)?;
            let mut counts = Counts::default();
            for row in &rows {
                counts.add_case(row);
            }
            Ok(
                json!({"total":rows.len(),"counts":counts,"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        "diagnostics" => {
            let rows: Vec<_> = report
                .diagnostics
                .iter()
                .filter(|d| args.filter.source.is_none_or(|id| id == d.source))
                .collect();
            Ok(
                json!({"total":rows.len(),"items":rows.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
            )
        }
        _ => Err(error("page")),
    }
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
