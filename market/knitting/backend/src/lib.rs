//! Knitting progress is validated and advanced in Rust. The UI persists the
//! returned workshop through the existing host SQLite plugin-state API.
mod export;
mod model;
mod ops;
mod view;

use model::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, to_value, unknown_function,
};
use view::Pages;

const MANIFEST: &str = include_str!("../../manifest.json");
pub struct Knitting {
    manifest: Manifest,
}
impl Default for Knitting {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    state: Workshop,
    request_id: String,
    #[serde(default)]
    pages: Pages,
    #[serde(default)]
    follow_selection: bool,
    op: Option<ops::Op>,
    path: Option<String>,
}

fn read_backup(path: &str, ctx: &dyn TaskContext) -> PluginResult<Workshop> {
    if ctx.file_size(path)? > MAX_EXPORT as u64 {
        return Err(error("state_limit"));
    }
    let mut reader = ctx.open_file(path)?.take(MAX_EXPORT as u64 + 1);
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        check_cancel(ctx)?;
        let n = reader.read(&mut buffer).map_err(|_| error("file"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() > MAX_EXPORT {
            return Err(error("state_limit"));
        }
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let state: Workshop = serde_json::from_slice(bytes).map_err(|_| error("invalid_state"))?;
    validate(&state, ctx)?;
    Ok(state)
}

impl ToolPlugin for Knitting {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        let _ = args;
        Err(unknown_function(function))
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if function == "export" {
            return export::save(parse_args(args)?, ctx);
        }
        if !["inspect", "apply", "restore"].contains(&function) {
            return Err(unknown_function(function));
        }
        let request: Request = parse_args(args)?;
        if request.request_id.is_empty() || request.request_id.len() > 100 {
            return Err(error("operation"));
        }
        ctx.stage("validate");
        validate(&request.state, ctx)?;
        let base_revision = request.state.revision;
        let changed = function != "inspect";
        let mut state = match function {
            "inspect" => request.state,
            "apply" => {
                ctx.stage("update");
                ops::apply(
                    request.state,
                    request.op.ok_or_else(|| error("operation"))?,
                    ctx,
                )?
            }
            "restore" => {
                ctx.stage("restore");
                let mut state = read_backup(&request.path.ok_or_else(|| error("file"))?, ctx)?;
                state.revision = base_revision
                    .checked_add(1)
                    .ok_or_else(|| error("limits"))?;
                state
            }
            _ => unreachable!(),
        };
        ctx.stage("view");
        let view = view::inspect(&state, request.pages, request.follow_selection, ctx)?;
        check_cancel(ctx)?;
        ctx.stage("done");
        let state = if changed {
            Some(std::mem::take(&mut state))
        } else {
            None
        };
        Ok(
            json!({"requestId":request.request_id,"baseRevision":base_revision,"state":state,"view":to_value(view)?}),
        )
    }
}

tf_plugin_guest::export_plugin!(Knitting);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
