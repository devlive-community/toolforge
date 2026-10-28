use super::*;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use tf_plugin_api::{LogLevel, PluginError};

#[derive(Default)]
pub struct Context {
    pub cancelled: AtomicBool,
    pub cancel_on: Option<&'static str>,
}
impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, code: &str) {
        if self.cancel_on == Some(code) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn Read + Send>> {
        Err(PluginError::new("test.file"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("test.file"))
    }
}
pub fn spec(seed: &str) -> Spec {
    Spec {
        algorithm: ALGORITHM.into(),
        rows: 8,
        cols: 10,
        entrance: Port {
            side: Side::Left,
            index: 1,
        },
        exit: Port {
            side: Side::Right,
            index: 8,
        },
        seed: seed.into(),
    }
}
pub fn initial() -> State {
    State {
        schema_version: 1,
        revision: 0,
        game: None,
    }
}
pub fn created() -> State {
    let mut state = initial();
    game::apply(
        &mut state,
        None,
        game::Op::ImportCode {
            code: spec("review").code(),
            confirm: true,
        },
        &Context::default(),
    )
    .unwrap();
    state
}
pub fn request(plugin: &MazeMaker, state: &mut State, op: Value) -> Value {
    let result = plugin
        .run_task(
            "apply",
            json!({"state":state,"requestId":"test","op":op}),
            &Context::default(),
        )
        .unwrap();
    if !result["state"].is_null() {
        *state = serde_json::from_value(result["state"].clone()).unwrap();
    }
    result
}

#[test]
fn canonical_state_roundtrip_preserves_position_trail_and_hint_counts() {
    let plugin = MazeMaker::default();
    let mut state = created();
    let hint = request(&plugin, &mut state, json!({"type":"hint"}));
    let target = state.game.as_ref().unwrap().hint_target.unwrap();
    let moved = request(&plugin, &mut state, json!({"type":"moveTo","cell":target}));
    assert_eq!(moved["view"]["steps"], 1);
    assert_eq!(hint["view"]["hints"], 1);
    let saved = serde_json::to_string(&state).unwrap();
    let state: State = serde_json::from_str(&saved).unwrap();
    let reopened = plugin
        .run_task(
            "inspect",
            json!({"state":state,"requestId":"reopen"}),
            &Context::default(),
        )
        .unwrap();
    for key in ["id", "position", "steps", "trailPath", "hints", "code"] {
        assert_eq!(moved["view"][key], reopened["view"][key]);
    }
    assert!(reopened["view"]["answerPath"].is_null());
}
#[test]
fn invalid_moves_keep_revision_and_valid_step_count() {
    let plugin = MazeMaker::default();
    let mut state = created();
    let old = serde_json::to_value(&state).unwrap();
    let result = request(&plugin, &mut state, json!({"type":"moveTo","cell":999999}));
    assert_eq!(result["notice"], "blocked");
    assert!(result["state"].is_null());
    assert_eq!(serde_json::to_value(&state).unwrap(), old);
    let result = request(&plugin, &mut state, json!({"type":"move","side":"left"}));
    assert_eq!(result["view"]["steps"], 0);
}
#[test]
fn cancellation_after_update_does_not_return_new_progress() {
    let plugin = MazeMaker::default();
    let state = created();
    let error = plugin
        .run_task(
            "apply",
            json!({"state":state,"requestId":"cancel","op":{"type":"hint"}}),
            &Context {
                cancel_on: Some("view"),
                ..Context::default()
            },
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "task.cancelled");
    assert_eq!(state.game.unwrap().hints, 0);
}
#[test]
fn exports_real_pdf_svg_png_and_rejects_unrecorded_answers() {
    let plugin = MazeMaker::default();
    let mut state = created();
    let ctx = Context::default();
    let dir = std::env::temp_dir().join(format!("tf-maze-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let no = plugin
        .run_task(
            "preview",
            json!({"state":state,"options":{"count":1,"perPage":1,"sheet":"answer"}}),
            &ctx,
        )
        .err()
        .unwrap();
    assert_eq!(no.code, "maze.answer_confirm");
    request(&plugin, &mut state, json!({"type":"markAnswer"}));
    for (format, sheet, count) in [
        ("pdf", "both", 3),
        ("svg", "puzzle", 1),
        ("png", "answer", 1),
    ] {
        let path = dir.join(format!("maze.{format}"));
        plugin.run_task("export",json!({"state":state,"path":path,"format":format,"options":{"count":count,"perPage":2,"sheet":sheet}}),&ctx).unwrap();
        let bytes = std::fs::read(path).unwrap();
        match format {
            "pdf" => assert_eq!(
                lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(),
                4
            ),
            "svg" => assert!(std::str::from_utf8(&bytes).unwrap().starts_with("<svg")),
            _ => assert!(image::load_from_memory(&bytes).is_ok()),
        }
    }
    let path = dir.join("cancelled.pdf");
    assert!(plugin.run_task("export",json!({"state":state,"path":path,"format":"pdf","options":{"count":1,"perPage":1,"sheet":"puzzle"}}),&Context{cancel_on:Some("export"),..Context::default()}).is_err());
    assert!(!path.exists());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn empty_state_unknown_functions_versions_and_parameters_fail_clearly() {
    let plugin = MazeMaker::default();
    let ctx = Context::default();
    assert!(plugin.call("apply", json!({})).is_err());
    assert!(plugin.run_task("missing", json!({}), &ctx).is_err());
    let empty = plugin
        .run_task(
            "inspect",
            json!({"state":initial(),"requestId":"empty"}),
            &ctx,
        )
        .unwrap();
    assert!(empty["view"].is_null());
    let mut state = created();
    state.schema_version = 2;
    assert!(state.validate(&ctx).is_err());
    assert!(
        plugin
            .run_task(
                "apply",
                json!({"state":initial(),"requestId":"empty","op":{"type":"hint"}}),
                &ctx
            )
            .is_err()
    );
}

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    let plugin = MazeMaker::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}
