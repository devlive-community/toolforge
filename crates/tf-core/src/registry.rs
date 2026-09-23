use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;
use tf_plugin_api::{Manifest, TaskContext, ToolPlugin};

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

    /// 查找插件并确认函数已在 manifest 中声明，返回插件与该函数是否为任务
    fn resolve(&self, plugin_id: &str, function: &str) -> AppResult<(Arc<dyn ToolPlugin>, bool)> {
        let plugin = self.get(plugin_id)?;
        let spec =
            plugin.manifest().functions.get(function).ok_or_else(|| {
                AppError::new("plugin.function_not_found").with("function", function)
            })?;
        let task = spec.task;
        Ok((plugin, task))
    }

    /// 普通调用；声明为任务的函数必须通过 [`Self::run_task`] 运行
    pub fn call(&self, plugin_id: &str, function: &str, args: Value) -> AppResult<Value> {
        let (plugin, task) = self.resolve(plugin_id, function)?;
        if task {
            return Err(AppError::new("plugin.requires_task").with("function", function));
        }
        Ok(plugin.call(function, args)?)
    }

    /// 确认函数可以作为任务运行（在启动任务线程前同步校验）
    pub fn ensure_task(&self, plugin_id: &str, function: &str) -> AppResult<()> {
        match self.resolve(plugin_id, function)? {
            (_, true) => Ok(()),
            _ => Err(AppError::new("plugin.not_a_task").with("function", function)),
        }
    }

    pub fn run_task(
        &self,
        plugin_id: &str,
        function: &str,
        args: Value,
        ctx: &dyn TaskContext,
    ) -> AppResult<Value> {
        self.ensure_task(plugin_id, function)?;
        let (plugin, _) = self.resolve(plugin_id, function)?;
        Ok(plugin.run_task(function, args, ctx)?)
    }
}

#[cfg(test)]
#[path = "registry_test.rs"]
mod tests;
