use super::*;
use serde_json::json;
use tf_plugin_api::{PluginResult, unknown_function};

struct Echo(Manifest);

impl ToolPlugin for Echo {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "echo" | "hidden" | "long" => Ok(args),
            other => Err(unknown_function(other)),
        }
    }
}

fn registry() -> PluginRegistry {
    let manifest = Manifest::from_static(
        r#"{"id":"echo","version":"1.0.0","name":"i18n:name","description":"i18n:d",
            "category":"dev","functions":{"echo":{},"long":{"task":true}}}"#,
    );
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(Echo(manifest)));
    registry
}

#[test]
fn calls_declared_function() {
    let out = registry().call("echo", "echo", json!({"a": 1})).unwrap();
    assert_eq!(out, json!({"a": 1}));
}

#[test]
fn rejects_undeclared_function() {
    let err = registry().call("echo", "hidden", json!(null)).unwrap_err();
    assert_eq!(err.code, "plugin.function_not_found");
}

#[test]
fn rejects_unknown_plugin() {
    let err = registry().call("missing", "echo", json!(null)).unwrap_err();
    assert_eq!(err.code, "plugin.not_found");
}

#[test]
fn task_functions_cannot_be_called_directly() {
    let err = registry().call("echo", "long", json!(null)).unwrap_err();
    assert_eq!(err.code, "plugin.requires_task");
}

#[test]
fn only_task_functions_can_run_as_tasks() {
    assert!(registry().ensure_task("echo", "long").is_ok());
    assert_eq!(
        registry().ensure_task("echo", "echo").unwrap_err().code,
        "plugin.not_a_task"
    );
}
