//! All scoring rules and rankings run in Rust. Returned states are committed
//! through the host's existing SQLite plugin-state API before UI publication.
mod export;
mod flat;
mod model;
mod ops;
mod view;

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, unknown_function,
};

/// 后端分发的全部函数；宿主只放行 manifest 中声明过的函数，测试据此核对两边一致
#[cfg(test)]
const FUNCTIONS: [&str; 4] = ["inspect", "apply", "export", "rank"];

pub struct BoardgameScore {
    manifest: Manifest,
}
impl Default for BoardgameScore {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    state: model::State,
    request_id: String,
    #[serde(default)]
    pages: view::Pages,
    #[serde(default)]
    follow: bool,
    op: Option<ops::Op>,
}
impl ToolPlugin for BoardgameScore {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "rank" => tf_plugin_api::to_value(flat::rank(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if function == "export" {
            return export::save(parse_args(args)?, ctx);
        }
        if !["inspect", "apply"].contains(&function) {
            return Err(unknown_function(function));
        }
        let request: Request = parse_args(args)?;
        if request.request_id.is_empty() || request.request_id.len() > 100 {
            return Err(model::error("stale"));
        }
        ctx.stage("validate");
        model::validate(&request.state, ctx)?;
        let base_revision = request.state.revision;
        let changed = function == "apply";
        let state = if changed {
            ctx.stage("update");
            ops::apply(
                request.state,
                request.op.ok_or_else(|| model::error("stale"))?,
                ctx,
            )?
        } else {
            request.state
        };
        ctx.stage("view");
        let view = view::inspect(&state, request.pages, request.follow, ctx)?;
        model::check(ctx)?;
        ctx.stage("done");
        Ok(
            json!({"requestId":request.request_id,"baseRevision":base_revision,"state":changed.then_some(state),"view":view}),
        )
    }
}
tf_plugin_guest::export_plugin!(BoardgameScore);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
