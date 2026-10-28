//! Personal calendar-day plans. Actual completion is separate from postponement.
mod date;
mod flat;
mod model;
mod ops;
mod photos;
mod view;
use model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, unknown_function,
};
/// 后端分发的全部函数；宿主只放行 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 9] = [
    "bootstrap",
    "today",
    "due",
    "inspect",
    "apply",
    "preview",
    "photos",
    "export",
    "restore",
];
pub struct PlantCare {
    manifest: Manifest,
}
impl Default for PlantCare {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
        }
    }
}
fn today() -> String {
    jiff::Zoned::now().date().to_string()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    state: Garden,
    today: String,
    request_id: String,
    #[serde(default)]
    query: view::Query,
    op: Option<ops::Op>,
    key: Option<String>,
    path: Option<String>,
    ids: Option<Vec<String>>,
    format: Option<String>,
    language: Option<String>,
}
impl ToolPlugin for PlantCare {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "bootstrap" => Ok(json!({"session":id(),"today":today()})),
            "today" => Ok(json!({"today":today()})),
            "due" => tf_plugin_api::to_value(flat::due(parse_args(args)?, &today())?),
            _ => Err(unknown_function(function)),
        }
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if !["inspect", "apply", "preview", "restore", "photos", "export"].contains(&function) {
            return Err(unknown_function(function));
        }
        let r: Request = parse_args(args)?;
        if r.request_id.is_empty() || r.request_id.len() > 120 {
            return Err(error("state"));
        }
        ctx.stage("validate");
        ctx.log(tf_plugin_api::LogLevel::Info, "validate", json!({}));
        day(&r.today)?;
        validate(&r.state, ctx)?;
        let base_revision = r.state.revision;
        if function == "photos" {
            let ids = r.ids.ok_or_else(|| error("photo"))?;
            if ids.is_empty() || ids.len() > 2 {
                return Err(error("photo"));
            }
            if ids.len() == 2 && ids[0] == ids[1] {
                return Err(error("photo"));
            }
            let mut list = ids
                .iter()
                .map(|id| {
                    r.state
                        .photos
                        .iter()
                        .find(|p| &p.id == id && Some(&p.plant_id) == r.state.selected_id.as_ref())
                        .ok_or_else(|| error("missing"))
                })
                .collect::<PluginResult<Vec<_>>>()?;
            list.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
            check(ctx)?;
            return Ok(json!({"photos":list}));
        }
        if function == "export" {
            ctx.stage("export");
            let content = match r.format.as_deref() {
                Some("json") => {
                    serde_json::to_string_pretty(&r.state).map_err(|_| error("file"))?
                }
                Some("markdown") => markdown(&r.state, r.language.as_deref().unwrap_or("en-US")),
                _ => return Err(error("file")),
            };
            if content.len() > 8 * 1024 * 1024 {
                return Err(error("storage"));
            }
            check(ctx)?;
            if let Some(path) = r.path {
                tf_plugin_guest::net::write_file(&path, content.as_bytes())?;
                return Ok(json!({"saved":true}));
            }
            if content.len() > 65536 {
                return Err(error("copy"));
            }
            return Ok(json!({"content":content}));
        }
        let preview_id = r.op.as_ref().and_then(|op| match op {
            ops::Op::EditPlan { plan, .. } => Some(plan.clone()),
            _ => None,
        });
        let mut state = match function {
            "apply" | "preview" => {
                ctx.stage("update");
                let op = r.op.ok_or_else(|| error("state"))?;
                if function == "preview" && !matches!(op, ops::Op::EditPlan { .. }) {
                    return Err(error("state"));
                }
                ops::apply(
                    r.state,
                    op,
                    r.key.as_deref().ok_or_else(|| error("state"))?,
                    &r.today,
                    ctx,
                )?
            }
            "restore" => {
                ctx.stage("restore");
                let path = r.path.ok_or_else(|| error("file"))?;
                if ctx.file_size(&path)? > 8 * 1024 * 1024 {
                    return Err(error("storage"));
                }
                let mut bytes = Vec::new();
                let mut reader = ctx.open_file(&path)?.take(8 * 1024 * 1024 + 1);
                let mut chunk = [0; 65536];
                loop {
                    check(ctx)?;
                    let n = reader.read(&mut chunk).map_err(|_| error("file"))?;
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if bytes.len() > 8 * 1024 * 1024 {
                        return Err(error("storage"));
                    }
                }
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err(error("storage"));
                }
                let mut g: Garden = serde_json::from_slice(&bytes).map_err(|_| error("state"))?;
                validate(&g, ctx)?;
                g.revision = base_revision
                    .checked_add(1)
                    .ok_or_else(|| error("limits"))?;
                g
            }
            _ => r.state,
        };
        if function == "restore" {
            validate(&state, ctx)?;
        }
        let preview_due = preview_id
            .as_ref()
            .and_then(|id| {
                state
                    .plants
                    .iter()
                    .flat_map(|p| &p.plans)
                    .find(|p| &p.id == id)
            })
            .map(|p| state.due(p))
            .transpose()?;
        ctx.stage("view");
        let view = view::inspect(&state, &r.today, &r.query, ctx)?;
        check(ctx)?;
        let changed =
            function != "inspect" && function != "preview" && state.revision != base_revision;
        Ok(
            json!({"requestId":r.request_id,"baseRevision":base_revision,"state":if changed{Some(std::mem::take(&mut state))}else{None},"view":view,"preview":function=="preview","previewDue":preview_due}),
        )
    }
}
fn markdown(g: &Garden, language: &str) -> String {
    fn escape(t: &str) -> String {
        let mut out = String::new();
        for c in t.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '\\' | '`' | '*' | '_' | '[' | ']' | '|' | '#' => {
                    out.push('\\');
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        out
    }
    let zh = language.starts_with("zh");
    let mut out = format!(
        "# {}\n\n",
        if zh {
            "植物养护日记"
        } else {
            "Plant care journal"
        }
    );
    for p in &g.plants {
        out.push_str(&format!(
            "## {}\n\n{} · {}\n\n{}\n\n",
            escape(&p.name),
            escape(&p.variety),
            escape(&p.location),
            escape(&p.note)
        ));
        for record in g.records.iter().filter(|r| r.plant_id == p.id) {
            let name = p
                .plans
                .iter()
                .find(|plan| plan.id == record.plan_id)
                .map_or("", |p| p.name.as_str());
            out.push_str(&format!(
                "- {} · {}{}: {}\n",
                record.date,
                escape(name),
                if record.voided {
                    if zh { " [已撤销]" } else { " [void]" }
                } else {
                    ""
                },
                escape(&record.note)
            ));
        }
        for photo in g.photos.iter().filter(|r| r.plant_id == p.id) {
            out.push_str(&format!(
                "- {} · {} {}: {}\n",
                photo.date,
                if zh { "照片" } else { "Photo" },
                photo.id,
                escape(&photo.note)
            ));
        }
        out.push('\n');
    }
    out
}
tf_plugin_guest::export_plugin!(PlantCare);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
