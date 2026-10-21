use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tf_plugin_api::{PluginResult, TaskContext};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pair {
    pub before: String,
    pub after: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    pub handle: String,
    pub kind: String,
    #[serde(default = "side")]
    pub side: String,
    pub output: Option<String>,
    pub other: Option<String>,
    pub module: Option<String>,
    #[serde(default)]
    pub pairs: Vec<Pair>,
    #[serde(default)]
    pub directory: Vec<String>,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "page_size")]
    pub limit: usize,
}
fn side() -> String {
    "before".into()
}
fn page_size() -> usize {
    25
}
impl Query {
    pub fn validate(&self) -> PluginResult<()> {
        if self.limit == 0
            || self.limit > 100
            || self.offset > 100_000
            || self.search.len() > 512
            || self.directory.len() > 24
            || self.pairs.len() > 2000
            || self
                .pairs
                .iter()
                .any(|p| p.before.len() > 2048 || p.after.len() > 2048)
            || self.directory.iter().any(|s| s.len() > 2048)
        {
            Err(error("query"))
        } else {
            Ok(())
        }
    }
}
pub fn out<'a>(s: &'a Snapshot, path: Option<&str>) -> PluginResult<&'a Output> {
    s.meta
        .outputs
        .get(path.ok_or_else(|| error("output"))?)
        .ok_or_else(|| error("output"))
}
pub fn sum(out: &Output) -> u64 {
    out.inputs.values().map(|c| c.bytes_in_output).sum()
}
pub fn output_row(path: &str, out: &Output) -> Value {
    let attributed = sum(out);
    json!({"path":path,"bytes":out.bytes,"entryPoint":out.entry_point,"cssBundle":out.css_bundle,"modules":out.inputs.len(),"imports":out.imports.len(),"attributed":attributed,"unattributed":out.bytes.saturating_sub(attributed),"overAttributed":attributed.saturating_sub(out.bytes)})
}
pub fn modules(
    s: &Snapshot,
    path: &str,
    search: &str,
    directory: &[String],
    ctx: &dyn TaskContext,
) -> PluginResult<Vec<Value>> {
    let out = out(s, Some(path))?;
    let needle = search.to_lowercase();
    let mut items = Vec::new();
    for (name, c) in &out.inputs {
        check(ctx)?;
        let parts = name
            .split(['/', '\\'])
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if !parts.starts_with(directory) || !name.to_lowercase().contains(&needle) {
            continue;
        }
        let input = s.meta.inputs.get(name);
        items.push(json!({"path":name,"bytes":c.bytes_in_output,"sourceBytes":input.map(|i|i.bytes),"known":input.is_some()}));
    }
    items.sort_by(|a, b| {
        b["bytes"]
            .as_u64()
            .cmp(&a["bytes"].as_u64())
            .then(a["path"].as_str().cmp(&b["path"].as_str()))
    });
    Ok(items)
}
type OutputPair = (Option<String>, Option<String>, &'static str);
pub fn pairs(reports: &Reports, manual: &[Pair]) -> PluginResult<Vec<OutputPair>> {
    let after = reports.after.as_ref().ok_or_else(|| error("after"))?;
    if manual.len() > 2000 {
        return Err(error("pair"));
    }
    let mut left = BTreeSet::new();
    let mut right = BTreeSet::new();
    let mut result = Vec::new();
    for p in manual {
        if !reports.before.meta.outputs.contains_key(&p.before)
            || !after.meta.outputs.contains_key(&p.after)
            || !left.insert(p.before.clone())
            || !right.insert(p.after.clone())
        {
            return Err(error("pair"));
        }
        result.push((Some(p.before.clone()), Some(p.after.clone()), "manual"));
    }
    for path in reports.before.meta.outputs.keys() {
        if left.contains(path) {
            continue;
        }
        left.insert(path.clone());
        if after.meta.outputs.contains_key(path) && !right.contains(path) {
            right.insert(path.clone());
            result.push((Some(path.clone()), Some(path.clone()), "exact"));
        } else {
            result.push((Some(path.clone()), None, "removed"));
        }
    }
    for path in after.meta.outputs.keys() {
        if !right.contains(path) {
            result.push((None, Some(path.clone()), "added"));
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(result)
}
pub fn comparison(reports: &Reports, q: &Query, ctx: &dyn TaskContext) -> PluginResult<Vec<Value>> {
    let after = reports.after.as_ref().ok_or_else(|| error("after"))?;
    let mut rows = Vec::new();
    let needle = q.search.to_lowercase();
    for (left, right, mode) in pairs(reports, &q.pairs)? {
        check(ctx)?;
        if !left
            .iter()
            .chain(right.iter())
            .any(|p| p.to_lowercase().contains(&needle))
        {
            continue;
        }
        let before = left.as_ref().map(|p| reports.before.meta.outputs[p].bytes);
        let next = right.as_ref().map(|p| after.meta.outputs[p].bytes);
        rows.push(json!({"before":left,"after":right,"mode":mode,"beforeBytes":before,"afterBytes":next,"delta":next.unwrap_or(0) as i64-before.unwrap_or(0) as i64}));
    }
    rows.sort_by(|a, b| {
        b["delta"]
            .as_i64()
            .unwrap()
            .unsigned_abs()
            .cmp(&a["delta"].as_i64().unwrap().unsigned_abs())
    });
    Ok(rows)
}
pub fn diff_modules(
    reports: &Reports,
    q: &Query,
    ctx: &dyn TaskContext,
) -> PluginResult<Vec<Value>> {
    let after = reports.after.as_ref().ok_or_else(|| error("after"))?;
    let left = q.output.as_ref();
    let right = q.other.as_ref();
    if !pairs(reports, &q.pairs)?
        .iter()
        .any(|(a, b, _)| a.as_ref() == left && b.as_ref() == right)
    {
        return Err(error("pair"));
    }
    let before = left.map(|p| &reports.before.meta.outputs[p]);
    let next = right.map(|p| &after.meta.outputs[p]);
    let mut names = BTreeSet::new();
    for out in [before, next].into_iter().flatten() {
        names.extend(out.inputs.keys());
    }
    let needle = q.search.to_lowercase();
    let mut rows = Vec::new();
    for name in names {
        check(ctx)?;
        if !name.to_lowercase().contains(&needle) {
            continue;
        }
        let b = before
            .and_then(|o| o.inputs.get(name))
            .map(|c| c.bytes_in_output);
        let a = next
            .and_then(|o| o.inputs.get(name))
            .map(|c| c.bytes_in_output);
        rows.push(json!({"path":name,"beforeBytes":b,"afterBytes":a,"delta":a.unwrap_or(0) as i64-b.unwrap_or(0) as i64}));
    }
    rows.sort_by(|a, b| {
        b["delta"]
            .as_i64()
            .unwrap()
            .unsigned_abs()
            .cmp(&a["delta"].as_i64().unwrap().unsigned_abs())
    });
    Ok(rows)
}
pub fn rows(reports: &Reports, q: &Query, ctx: &dyn TaskContext) -> PluginResult<Vec<Value>> {
    q.validate()?;
    if q.kind == "pairings" {
        return Ok(q
            .pairs
            .iter()
            .enumerate()
            .map(|(id, p)| json!({"id":id,"before":p.before,"after":p.after}))
            .collect());
    }
    if q.kind == "compare" {
        return comparison(reports, q, ctx);
    }
    if q.kind == "diffModules" {
        return diff_modules(reports, q, ctx);
    }
    let s = reports.side(&q.side)?;
    let needle = q.search.to_lowercase();
    match q.kind.as_str() {
        "trace" => {
            let evidence = trace(
                s,
                q.output.as_deref().ok_or_else(|| error("output"))?,
                q.module.as_deref().ok_or_else(|| error("module"))?,
                ctx,
            )?;
            let mut rows = evidence["edges"].as_array().unwrap().clone();
            if rows.is_empty() {
                rows.push(
                    json!({"module":q.module,"root":evidence["root"],"state":evidence["state"]}),
                );
            } else {
                for row in &mut rows {
                    row["root"] = evidence["root"].clone();
                    row["state"] = evidence["state"].clone();
                }
            }
            Ok(rows)
        }

        "output" => Ok(vec![output_row(
            q.output.as_deref().ok_or_else(|| error("output"))?,
            out(s, q.output.as_deref())?,
        )]),
        "outputs" => {
            let mut rows = s
                .meta
                .outputs
                .iter()
                .filter(|(p, _)| p.to_lowercase().contains(&needle))
                .map(|(p, o)| output_row(p, o))
                .collect::<Vec<_>>();
            rows.sort_by(|a, b| b["bytes"].as_u64().cmp(&a["bytes"].as_u64()));
            Ok(rows)
        }
        "modules" => modules(
            s,
            q.output.as_deref().ok_or_else(|| error("output"))?,
            &q.search,
            &q.directory,
            ctx,
        ),
        "diagnostics" => Ok(s
            .diagnostics
            .iter()
            .map(|d| serde_json::to_value(d).unwrap())
            .collect()),
        "imports" => {
            let imports = if let Some(name) = &q.module {
                &s.meta
                    .inputs
                    .get(name)
                    .ok_or_else(|| error("module"))?
                    .imports
            } else {
                &out(s, q.output.as_deref())?.imports
            };
            let known = |p: &str| {
                if q.module.is_some() {
                    s.meta.inputs.contains_key(p)
                } else {
                    s.meta.outputs.contains_key(p)
                }
            };
            Ok(imports.iter().map(|i|json!({"path":i.path,"kind":i.kind,"external":i.external,"resolved":!i.external&&known(&i.path)})).collect())
        }
        _ => Err(error("query")),
    }
}
pub fn trace(
    s: &Snapshot,
    output: &str,
    module: &str,
    ctx: &dyn TaskContext,
) -> PluginResult<Value> {
    if !out(s, Some(output))?.inputs.contains_key(module) {
        return Err(error("module"));
    }
    let selected = &s.meta.outputs[output];
    let roots: Vec<String> = if let Some(root) = &selected.entry_point {
        vec![root.clone()]
    } else {
        s.meta
            .outputs
            .values()
            .filter_map(|o| o.entry_point.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let mut seen = BTreeMap::<String, Option<(String, String)>>::new();
    let mut queue = VecDeque::new();
    for root in roots {
        if s.meta.inputs.contains_key(&root) {
            seen.insert(root.clone(), None);
            queue.push_back((root, 0usize));
        }
    }
    let mut limited = false;
    while let Some((at, depth)) = queue.pop_front() {
        check(ctx)?;
        if at == module {
            let mut edges = Vec::new();
            let mut current = at.clone();
            while let Some(Some((parent, kind))) = seen.get(&current) {
                edges.push(json!({"from":parent,"to":current,"kind":kind}));
                current = parent.clone();
            }
            edges.reverse();
            return Ok(json!({"state":"found","root":current,"edges":edges,"module":module}));
        }
        if depth >= 64 {
            limited = true;
            continue;
        }
        for edge in &s.meta.inputs[&at].imports {
            check(ctx)?;
            if !edge.external
                && s.meta.inputs.contains_key(&edge.path)
                && !seen.contains_key(&edge.path)
            {
                seen.insert(edge.path.clone(), Some((at.clone(), edge.kind.clone())));
                queue.push_back((edge.path.clone(), depth + 1));
            }
        }
    }
    Ok(json!({"state":if limited{"limited"}else{"unavailable"},"edges":[],"module":module}))
}
