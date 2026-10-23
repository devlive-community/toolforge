use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde_json::{Value, json};
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext, ToolPlugin};

use super::JsonSchema;

#[derive(Default)]
pub(crate) struct Context {
    pub cancelled: AtomicBool,
    pub file_reads: AtomicUsize,
}

impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn progress(&self, _: u64, _: u64) {}
    fn stage(&self, _: &str) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    fn open_file(&self, _: &str) -> PluginResult<Box<dyn std::io::Read + Send>> {
        self.file_reads.fetch_add(1, Ordering::SeqCst);
        Err(PluginError::new("test.forbidden_io"))
    }
    fn file_size(&self, _: &str) -> PluginResult<u64> {
        Err(PluginError::new("test.forbidden_io"))
    }
}

#[test]
fn manifest_exposes_task_and_user_selected_files_without_network_permission() {
    let plugin = JsonSchema::default();
    assert!(plugin.manifest().functions["validate"].task);
    assert_eq!(plugin.manifest().category, "dev");
    assert_eq!(
        plugin.manifest().permissions,
        ["fs:user-selected", "clipboard:write"]
    );
    assert!(plugin.call("validate", json!({})).is_err());
    assert!(
        plugin
            .run_task("missing", json!({}), &Context::default())
            .is_err()
    );
}

#[test]
fn detects_only_explicit_supported_schema_declarations() {
    let plugin = JsonSchema::default();
    assert!(
        plugin
            .detect(r#"{"type":"object","properties":{"id":{"type":"integer"}}}"#)
            .is_none()
    );
    assert!(plugin.detect(r#"{"hello":"world"}"#).is_none());
    assert!(
        plugin
            .detect(r#"{"$schema":"https://evil.test/schema"}"#)
            .is_none()
    );
    assert!(plugin.detect(r#"{"$schema":42}"#).is_none());
    assert!(plugin.detect("true").is_none());
    let detected = plugin
        .detect(r#"{"$schema":"https://json-schema.org/draft/2020-12/schema"}"#)
        .unwrap();
    assert_eq!(detected.label, "schema");
    assert_eq!(detected.score, 96);
    assert_eq!(
        plugin
            .detect(r#"{"$schema":"http://json-schema.org/draft-07/schema#"}"#)
            .unwrap()
            .label,
        "schema7"
    );
}

#[test]
fn task_contract_returns_structured_diagnostics() {
    let ctx = Context::default();
    let result = JsonSchema::default()
        .run_task(
            "validate",
            json!({
                "instance": "{\"count\":\"wrong\"}",
                "schema": "{\"properties\":{\"count\":{\"type\":\"integer\"}}}",
                "draft": "2020-12",
                "validateFormats": false,
            }),
            &ctx,
        )
        .unwrap();
    assert_eq!(result["valid"], false);
    assert_eq!(result["schemaValid"], true);
    assert_eq!(result["diagnostics"][0]["instancePath"], "/count");
    assert_eq!(
        result["diagnostics"][0]["schemaPath"],
        "/properties/count/type"
    );
    assert_eq!(ctx.file_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&JsonSchema::default()).unwrap();
    assert_eq!(checked, 1);
}
