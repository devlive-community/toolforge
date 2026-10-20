//! Local training records with recoverable, sharded shared-database storage.
mod engine;
mod model;
mod transfer;

use serde_json::{Value, json};
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

const MANIFEST: &str = include_str!("../../manifest.json");
pub struct WorkoutLog {
    manifest: Manifest,
}
impl Default for WorkoutLog {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}
impl ToolPlugin for WorkoutLog {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "plan" => to_value(engine::plan(parse_args(args)?)?),
            "export" => transfer::export(parse_args(args)?),
            "import" => to_value(transfer::import(parse_args(args)?)?),
            "recover" => transfer::recover(parse_args(args)?),
            "clock" => {
                #[derive(serde::Deserialize)]
                struct Args {
                    rest: Option<model::Rest>,
                }
                let args: Args = parse_args(args)?;
                if let Some(rest) = &args.rest {
                    model::validate_rest(rest)?;
                }
                let now = jiff::Timestamp::now().as_second();
                let remaining = args.rest.filter(|r| !r.stopped).map(|r| {
                    (r.started + r.seconds as i64)
                        .saturating_sub(now)
                        .clamp(0, r.seconds as i64)
                });
                Ok(
                    json!({"stamp":jiff::Timestamp::now().as_millisecond().to_string(),"remaining":remaining,"today":jiff::Zoned::now().date().to_string()}),
                )
            }
            other => Err(unknown_function(other)),
        }
    }
}
tf_plugin_guest::export_plugin!(WorkoutLog);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
