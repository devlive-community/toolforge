use super::*;
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
    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| PluginError::new("test.file"))
    }
    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|_| PluginError::new("test.file"))
    }
}
pub fn change(state: Workshop, op: Value) -> PluginResult<Workshop> {
    ops::apply(
        state,
        serde_json::from_value(op).unwrap(),
        &Context::default(),
    )
}
pub fn created() -> Workshop {
    change(Workshop::default(),json!({"type":"newProject","name":"Scarf","partName":"Front","kind":"rows","stitches":"40"})).unwrap()
}
fn run(plugin: &Knitting, function: &str, state: &Workshop, extra: Value) -> PluginResult<Value> {
    let mut args = json!({"state":state,"requestId":"r1"});
    for (k, v) in extra.as_object().unwrap() {
        args[k] = v.clone();
    }
    plugin.run_task(function, args, &Context::default())
}

#[test]
fn tasks_return_camel_case_fields_and_revision_for_atomic_client_persistence() {
    let plugin = Knitting::default();
    let state = Workshop::default();
    let out=run(&plugin,"apply",&state,json!({"op":{"type":"newProject","name":"Sample","partName":"Sleeve","kind":"rounds","stitches":"24"},"followSelection":true})).unwrap();
    assert_eq!(out["baseRevision"], 0);
    assert_eq!(out["requestId"], "r1");
    assert_eq!(out["state"]["revision"], 1);
    assert!(out["state"].get("activeProjectId").is_some());
    assert_eq!(out["view"]["part"]["nextLine"], 1);
    assert_eq!(out["view"]["part"]["kind"], "rounds");
    let state: Workshop = serde_json::from_value(out["state"].clone()).unwrap();
    let view = run(&plugin, "inspect", &state, json!({})).unwrap();
    assert_eq!(view["state"], Value::Null);
    assert_eq!(view["view"]["part"]["stitches"], 24);
    assert!(
        plugin
            .run_task("missing", json!({}), &Context::default())
            .is_err()
    );
    assert!(plugin.call("missing", json!({})).is_err());
}

#[test]
fn paginates_projects_parts_rules_and_history_without_losing_selection() {
    let mut state = created();
    for i in 0..6 {
        state = change(
            state,
            json!({"type":"newPart","name":format!("Piece {i}"),"kind":"rows","stitches":"1"}),
        )
        .unwrap();
    }
    for _ in 0..7 {
        let n = state.active().unwrap().1.completed + 1;
        state = change(state, json!({"type":"complete","expectedLine":n})).unwrap();
    }
    let view = view::inspect(&state, Pages::default(), true, &Context::default()).unwrap();
    assert_eq!(view.parts.total, 7);
    assert_eq!(view.parts.offset, 5);
    assert_eq!(view.parts.items.len(), 2);
    let pages = Pages {
        history: 5,
        ..Pages::default()
    };
    let view = view::inspect(&state, pages, false, &Context::default()).unwrap();
    assert_eq!(view.part.unwrap().history.items.len(), 2);
    for i in 0..6 {
        state=change(state,json!({"type":"newProject","name":format!("Project {i}"),"partName":"Body","kind":"rows","stitches":"1"})).unwrap();
    }
    let view = view::inspect(&state, Pages::default(), true, &Context::default()).unwrap();
    assert_eq!(
        (
            view.projects.total,
            view.projects.offset,
            view.projects.items.len()
        ),
        (7, 5, 2)
    );
    for _ in 0..6 {
        state=change(state,json!({"type":"addReminder","name":"Due","start":"1","interval":"1","repeats":"1","delta":"1"})).unwrap();
    }
    let mut view = view::inspect(&state, Pages::default(), false, &Context::default()).unwrap();
    assert_eq!(view.part.as_ref().unwrap().due.total, 6);
    for row in view.part.take().unwrap().due.items {
        state = change(
            state,
            json!({"type":"choose","id":row.id,"choice":"execute"}),
        )
        .unwrap();
    }
    let view = view::inspect(
        &state,
        Pages {
            due: 5,
            ..Pages::default()
        },
        false,
        &Context::default(),
    )
    .unwrap();
    let part = view.part.unwrap();
    assert_eq!(part.pending_count, 1);
    assert!(!part.can_complete);
    assert_eq!(part.due.items.len(), 1);
}

#[test]
fn cancellation_never_publishes_a_mutated_state() {
    let ctx = Context::default();
    ctx.cancelled.store(true, Ordering::Relaxed);
    let state = created();
    let before = state.clone();
    let result = Knitting::default().run_task(
        "apply",
        json!({"state":state,"requestId":"cancel","op":{"type":"complete","expectedLine":1}}),
        &ctx,
    );
    assert_eq!(result.unwrap_err().code, "task.cancelled");
    assert_eq!(state, before);
}

#[test]
fn export_restore_preserves_selection_rules_decisions_and_undo() {
    let mut state = created();
    state=change(state,json!({"type":"addReminder","name":"Shape","start":"1","interval":"2","repeats":"4","delta":"2"})).unwrap();
    let rule = state.active().unwrap().1.reminders[0].id.clone();
    state = change(state, json!({"type":"choose","id":rule,"choice":"execute"})).unwrap();
    state = change(state, json!({"type":"complete","expectedLine":1})).unwrap();
    let dir = std::env::temp_dir().join(format!("tf-knitting-roundtrip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("backup.json");
    let plugin = Knitting::default();
    let ctx = Context::default();
    plugin
        .run_task(
            "export",
            json!({"state":state,"format":"json","path":path,"language":"en-US"}),
            &ctx,
        )
        .unwrap();
    let backup: Workshop = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(backup, state);
    let restored = run(
        &plugin,
        "restore",
        &Workshop::default(),
        json!({"path":path,"followSelection":true}),
    )
    .unwrap();
    let mut actual: Workshop = serde_json::from_value(restored["state"].clone()).unwrap();
    actual.revision = state.revision;
    assert_eq!(actual, state);
    let resumed = change(actual, json!({"type":"undo"})).unwrap();
    assert_eq!(resumed.active().unwrap().1.stitches, 40);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn localized_markdown_escapes_notes_and_does_not_claim_planned_stitches() {
    let mut state = created();
    state=change(state,json!({"type":"details","projectName":"Scarf","partName":"Front","note":"<script> **my notes**"})).unwrap();
    let plugin = Knitting::default();
    let ctx = Context::default();
    for (language, label) in [("zh-CN", "当前针数"), ("en-US", "Current stitches")] {
        let result = plugin
            .run_task(
                "export",
                json!({"state":state,"format":"markdown","language":language,"partOnly":true}),
                &ctx,
            )
            .unwrap();
        let text = result["content"].as_str().unwrap();
        assert!(text.contains(label));
        assert!(text.contains("&lt;script&gt;"));
        assert!(!text.contains("<script>"));
        assert!(text.contains("40"));
    }
    assert!(
        plugin
            .run_task(
                "export",
                json!({"state":state,"format":"json","language":"en-US"}),
                &ctx
            )
            .is_err()
    );
}

#[test]
fn restore_rejects_unknown_versions_invalid_state_and_oversized_files() {
    let dir = std::env::temp_dir().join(format!("tf-knitting-reject-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("backup.json");
    let plugin = Knitting::default();
    std::fs::write(&path, b"{broken").unwrap();
    assert!(run(&plugin, "restore", &created(), json!({"path":path})).is_err());
    let mut state = created();
    state.schema_version = 2;
    std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(
        run(&plugin, "restore", &created(), json!({"path":path}))
            .unwrap_err()
            .code,
        "knitting.version"
    );
    std::fs::File::create(&path)
        .unwrap()
        .set_len(MAX_EXPORT as u64 + 1)
        .unwrap();
    assert_eq!(
        run(&plugin, "restore", &created(), json!({"path":path}))
            .unwrap_err()
            .code,
        "knitting.state_limit"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancellation_after_the_update_still_returns_no_state_to_save() {
    let ctx = Context {
        cancel_on: Some("view"),
        ..Context::default()
    };
    let state = created();
    let result=Knitting::default().run_task("apply",json!({"state":state,"requestId":"cancel-after-update","op":{"type":"complete","expectedLine":1}}),&ctx);
    assert_eq!(result.unwrap_err().code, "task.cancelled");
    assert_eq!(state.active().unwrap().1.completed, 0);
}

#[test]
fn utf8_bom_restore_keeps_uncommitted_reminder_choices_without_applying_them() {
    let mut state = created();
    state=change(state,json!({"type":"addReminder","name":"加针","start":"1","interval":"4","repeats":"8","delta":"2"})).unwrap();
    let id = state.active().unwrap().1.reminders[0].id.clone();
    state = change(state, json!({"type":"choose","id":id,"choice":"execute"})).unwrap();
    let path =
        std::env::temp_dir().join(format!("tf-knitting-pending-{}.json", std::process::id()));
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(serde_json::to_vec(&state).unwrap());
    std::fs::write(&path, bytes).unwrap();
    let out = run(
        &Knitting::default(),
        "restore",
        &Workshop::default(),
        json!({"path":path}),
    )
    .unwrap();
    assert_eq!(out["view"]["part"]["completed"], 0);
    assert_eq!(out["view"]["part"]["stitches"], 40);
    assert_eq!(out["view"]["part"]["projectedStitches"], 42);
    assert_eq!(out["view"]["part"]["due"]["items"][0]["choice"], "execute");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    let plugin = Knitting::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}

#[test]
fn declared_interfaces_are_accepted_by_backend() {
    let plugin = Knitting::default();
    let checked = tf_plugin_api::interface::check_declared_interfaces(&plugin)
        .expect("declared interfaces must match the backend");
    assert!(checked >= 1);
}
