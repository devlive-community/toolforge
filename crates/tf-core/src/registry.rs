use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;
use tf_plugin_api::{Manifest, ToolPlugin};

use crate::{AppError, AppResult};

/// 已加载插件的注册表。宿主只通过这里访问插件，不直接依赖任何具体工具。
#[derive(Default)]
pub struct PluginRegistry {
    plugins: IndexMap<String, Arc<dyn ToolPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plugin: Arc<dyn ToolPlugin>) {
        let id = plugin.manifest().id.clone();
        self.plugins.insert(id, plugin);
    }

    pub fn manifests(&self) -> Vec<Manifest> {
        self.plugins
            .values()
            .map(|p| p.manifest().clone())
            .collect()
    }

    pub fn get(&self, plugin_id: &str) -> AppResult<Arc<dyn ToolPlugin>> {
        self.plugins
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| AppError::new("plugin.not_found").with("plugin", plugin_id))
    }

    /// 调用插件函数；只允许调用 manifest 中声明过的函数。
    pub fn call(&self, plugin_id: &str, function: &str, args: Value) -> AppResult<Value> {
        let plugin = self.get(plugin_id)?;
        if !plugin.manifest().functions.contains_key(function) {
            return Err(AppError::new("plugin.function_not_found").with("function", function));
        }
        Ok(plugin.call(function, args)?)
    }
}

#[cfg(test)]
mod tests {
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
                "echo" | "hidden" => Ok(args),
                other => Err(unknown_function(other)),
            }
        }
    }

    fn registry() -> PluginRegistry {
        let manifest = Manifest::from_static(
            r#"{"id":"echo","version":"1.0.0","name":"i18n:name","description":"i18n:d",
                "category":"dev","functions":{"echo":{}}}"#,
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
}
