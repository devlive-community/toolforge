use super::*;
use tf_plugin_api::LogLevel;
use tf_plugin_api::ToolPlugin;

/// 测试用的任务上下文：记录阶段与取消状态
struct Ctx {
    cancelled: bool,
}

impl TaskContext for Ctx {
    fn log(&self, _level: LogLevel, _code: &str, _params: Value) {}
    fn progress(&self, _done: u64, _total: u64) {}
    fn stage(&self, _code: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn open_file(&self, _path: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        Err(PluginError::new("fs.denied"))
    }
    fn file_size(&self, _path: &str) -> PluginResult<u64> {
        Err(PluginError::new("fs.denied"))
    }
}

const SAMPLE: &str = r#"{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature", "properties": {"name": "a"}, "geometry": {"type": "Point", "coordinates": [1.0, 2.0]}}
  ]
}"#;

#[test]
fn inspect_returns_handle_and_stats() {
    let plugin = GeoJsonInspector::default();
    let out = plugin
        .run_task("inspect", json_args(SAMPLE), &Ctx { cancelled: false })
        .unwrap();
    assert_eq!(out["handle"], 1);
    assert_eq!(out["overall"], "passed");
    assert_eq!(out["stats"]["features"], 1);
    assert_eq!(out["stats"]["positions"], 1);
    let out = plugin
        .run_task("inspect", json_args(SAMPLE), &Ctx { cancelled: false })
        .unwrap();
    assert_eq!(out["handle"], 2);
}

fn json_args(text: &str) -> Value {
    serde_json::json!({ "text": text })
}

#[test]
fn syntax_error_is_reported_with_position() {
    let plugin = GeoJsonInspector::default();
    let out = plugin
        .run_task("inspect", json_args("{type:}"), &Ctx { cancelled: false })
        .unwrap();
    assert_eq!(out["overall"], "invalid");
    let diag = &out["diagnostics"][0];
    assert_eq!(diag["code"], "geojson.syntax");
    assert!(diag["line"].as_u64().unwrap() >= 1);
}

#[test]
fn export_requires_previous_inspect() {
    let plugin = GeoJsonInspector::default();
    let err = plugin
        .call("export", serde_json::json!({"format": "markdown"}))
        .unwrap_err();
    assert_eq!(err.code, "geojson.no_result");
}

#[test]
fn export_renders_markdown_and_json() {
    let plugin = GeoJsonInspector::default();
    plugin
        .run_task("inspect", json_args(SAMPLE), &Ctx { cancelled: false })
        .unwrap();
    let out = plugin
        .call("export", serde_json::json!({"format": "markdown"}))
        .unwrap();
    assert_eq!(out["ext"], "md");
    assert!(
        out["content"]
            .as_str()
            .unwrap()
            .contains("# GeoJSON Inspection")
    );
    let out = plugin
        .call("export", serde_json::json!({"format": "json"}))
        .unwrap();
    let parsed: Value = serde_json::from_str(out["content"].as_str().unwrap()).unwrap();
    assert_eq!(parsed["overall"], "passed");
}

#[test]
fn cancelled_inspection_returns_task_cancelled() {
    let plugin = GeoJsonInspector::default();
    let err = plugin
        .run_task("inspect", json_args(SAMPLE), &Ctx { cancelled: true })
        .unwrap_err();
    assert_eq!(err.code, "task.cancelled");
}

#[test]
fn oversized_input_is_rejected() {
    let plugin = GeoJsonInspector::default();
    let text = " ".repeat(spans::MAX_INPUT + 1);
    let err = plugin
        .run_task("inspect", json_args(&text), &Ctx { cancelled: false })
        .unwrap_err();
    assert_eq!(err.code, "geojson.too_large");
}

#[test]
fn unknown_function_is_rejected() {
    let err = GeoJsonInspector::default()
        .call("send", Value::Null)
        .unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}

/// Markdown 导出包含结论与诊断清单
#[test]
fn markdown_lists_diagnostics_with_pointers() {
    let plugin = GeoJsonInspector::default();
    plugin
        .run_task(
            "inspect",
            json_args(r#"{"type": "Feature", "properties": null, "geometry": {"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0]]]}}"#),
            &Ctx { cancelled: false },
        )
        .unwrap();
    let out = plugin
        .call("export", serde_json::json!({"format": "markdown"}))
        .unwrap();
    let text = out["content"].as_str().unwrap();
    assert!(text.contains("geojson.ring_not_closed"));
    assert!(text.contains("`/geometry/coordinates/0`"));
}
