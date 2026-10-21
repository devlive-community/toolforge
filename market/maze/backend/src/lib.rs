//! Maze generation, legal play, and print geometry stay in Rust. The UI saves
//! acknowledged state through ToolForge's existing SQLite plugin-state API.
mod drawing;
mod game;
mod maze;
mod paper;
use game::State;
use maze::*;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginResult, TaskContext, ToolPlugin, parse_args, unknown_function,
};

pub struct MazeMaker {
    manifest: Manifest,
}
impl Default for MazeMaker {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(include_str!("../../manifest.json")),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    state: State,
    request_id: String,
    op: Option<game::Op>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaperRequest {
    state: State,
    options: paper::Options,
    #[serde(default)]
    page: usize,
    format: Option<String>,
    path: Option<String>,
}
impl MazeMaker {
    fn paper(
        &self,
        function: &str,
        args: PaperRequest,
        ctx: &dyn TaskContext,
    ) -> PluginResult<Value> {
        args.state.validate(ctx)?;
        let g = args.state.game.ok_or_else(|| error("no_game"))?;
        if args.options.sheet != "puzzle" && !g.revealed {
            return Err(error("answer_confirm"));
        }
        ctx.stage("paper");
        let both = args.options.sheet == "both";
        let mut options = args.options.clone();
        if both {
            options.sheet = "puzzle".into();
        }
        let first = paper::plan(&g.spec, &options, ctx)?;
        if function == "preview" {
            if args.page > 11 {
                return Err(error("page"));
            }
            if both {
                return Err(error("paper"));
            }
            check(ctx)?;
            return Ok(
                json!({"svg":first.preview(args.page)?,"page":args.page,"total":first.pages(),"ids":first.ids.iter().skip(args.page*first.per_page).take(first.per_page).collect::<Vec<_>>(),"answer":first.answer}),
            );
        }
        let format = args.format.ok_or_else(|| error("format"))?;
        let data = match format.as_str() {
            "svg" | "png" => {
                if options.count != 1 || both {
                    return Err(error("paper"));
                }
                if format == "svg" {
                    drawing::svg(&first.scenes[0]).into_bytes()
                } else {
                    paper::png(&first.scenes[0], ctx)?
                }
            }
            "pdf" => {
                let mut plans = vec![first];
                if both {
                    options.sheet = "answer".into();
                    plans.push(paper::plan(&g.spec, &options, ctx)?);
                }
                paper::pdf(&plans, ctx)?
            }
            _ => return Err(error("format")),
        };
        if data.len() > 64 * 1024 * 1024 {
            return Err(error("limits"));
        }
        check(ctx)?;
        ctx.stage("export");
        check(ctx)?;
        tf_plugin_guest::net::write_file(&args.path.ok_or_else(|| error("file"))?, &data)?;
        Ok(json!({"saved":true,"bytes":data.len()}))
    }
}
impl ToolPlugin for MazeMaker {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    fn call(&self, function: &str, _: Value) -> PluginResult<Value> {
        Err(unknown_function(function))
    }
    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        check(ctx)?;
        if ["preview", "export"].contains(&function) {
            return self.paper(function, parse_args(args)?, ctx);
        }
        if !["inspect", "apply"].contains(&function) {
            return Err(unknown_function(function));
        }
        let mut request: Request = parse_args(args)?;
        if request.request_id.is_empty() || request.request_id.len() > 80 {
            return Err(error("state"));
        }
        ctx.stage("validate");
        let maze = request.state.validate(ctx)?;
        let base = request.state.revision;
        let (changed, notice) = if function == "apply" {
            ctx.stage("update");
            game::apply(
                &mut request.state,
                maze.as_ref(),
                request.op.ok_or_else(|| error("operation"))?,
                ctx,
            )?
        } else {
            (false, "ready")
        };
        if changed {
            request.state.revision += 1;
            if request.state.revision >= 1_000_000_000 {
                return Err(error("limits"));
            }
        }
        ctx.stage("view");
        let view = if let Some(g) = &request.state.game {
            let maze = generate(g.spec.clone(), ctx)?;
            Some(drawing::view(g, &maze, ctx)?)
        } else {
            None
        };
        check(ctx)?;
        Ok(
            json!({"requestId":request.request_id,"baseRevision":base,"state":if changed {Some(request.state)}else{None},"view":view,"notice":notice}),
        )
    }
}
tf_plugin_guest::export_plugin!(MazeMaker);
#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
