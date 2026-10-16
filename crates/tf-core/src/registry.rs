use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockReadGuard};

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use tf_plugin_api::{DETECT_LIMIT, Detection, Manifest, ServiceContext, TaskContext, ToolPlugin};

use crate::{AppError, AppResult};

/// 按剪贴板内容推荐的工具
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub plugin_id: String,
    #[serde(flatten)]
    pub detection: Detection,
}

/// 单行预览：合并空白、截断到 max 个字符
pub fn preview(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for word in text.split_whitespace() {
        if count >= max {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
            count += 1;
        }
        for c in word.chars().filter(|c| !c.is_control()) {
            if count >= max {
                break;
            }
            out.push(c);
            count += 1;
        }
    }
    if count >= max
        && text
            .split_whitespace()
            .map(|w| w.chars().count() + 1)
            .sum::<usize>()
            > max + 1
    {
        out.push('…');
    }
    out
}

struct ServiceDir(PathBuf);

impl ServiceContext for ServiceDir {
    fn data_dir(&self) -> &Path {
        &self.0
    }
}

/// 推荐的最低分数，低于它的识别结果不展示
const MIN_SCORE: u8 = 30;

/// 已加载插件的注册表。宿主只通过这里访问插件，不直接依赖任何具体工具。
/// 内置插件在启动时注册；在线安装的插件在运行中注册与卸载，因此内部加读写锁。
#[derive(Default)]
pub struct PluginRegistry {
    plugins: RwLock<IndexMap<String, Arc<dyn ToolPlugin>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, IndexMap<String, Arc<dyn ToolPlugin>>> {
        self.plugins.read().unwrap_or_else(|e| e.into_inner())
    }

    /// 注册插件；不支持当前系统的插件（如只适用于 Windows 的工具）直接跳过
    pub fn register(&self, plugin: Arc<dyn ToolPlugin>) {
        self.register_for(plugin, std::env::consts::OS);
    }

    pub fn register_for(&self, plugin: Arc<dyn ToolPlugin>, os: &str) {
        if !plugin.manifest().supports(os) {
            return;
        }
        let id = plugin.manifest().id.clone();
        self.plugins
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, plugin);
    }

    /// 卸载插件（在线安装的插件被移除或更新时），返回是否存在
    pub fn unregister(&self, plugin_id: &str) -> bool {
        self.plugins
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .shift_remove(plugin_id)
            .is_some()
    }

    pub fn contains(&self, plugin_id: &str) -> bool {
        self.read().contains_key(plugin_id)
    }

    pub fn manifests(&self) -> Vec<Manifest> {
        self.read().values().map(|p| p.manifest().clone()).collect()
    }

    pub fn get(&self, plugin_id: &str) -> AppResult<Arc<dyn ToolPlugin>> {
        self.read()
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

    /// 询问每个插件能否处理这段文本，按分数从高到低返回
    pub fn detect(&self, text: &str) -> Vec<Suggestion> {
        let text = text.trim();
        if text.is_empty() || text.len() > DETECT_LIMIT {
            return Vec::new();
        }
        let mut suggestions: Vec<Suggestion> = self
            .snapshot()
            .into_iter()
            .filter_map(|(id, plugin)| {
                plugin
                    .detect(text)
                    .filter(|d| d.score >= MIN_SCORE)
                    .map(|detection| Suggestion {
                        plugin_id: id,
                        detection,
                    })
            })
            .collect();
        // 稳定排序：同分时保持注册顺序
        suggestions.sort_by_key(|s| std::cmp::Reverse(s.detection.score));
        suggestions
    }

    /// 剪贴板中是图片时，询问哪些插件可以处理
    pub fn detect_image(&self, width: u32, height: u32) -> Vec<Suggestion> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let mut suggestions: Vec<Suggestion> = self
            .snapshot()
            .into_iter()
            .filter_map(|(id, plugin)| {
                plugin
                    .detect_image(width, height)
                    .filter(|d| d.score >= MIN_SCORE)
                    .map(|detection| Suggestion {
                        plugin_id: id,
                        detection,
                    })
            })
            .collect();
        suggestions.sort_by_key(|s| std::cmp::Reverse(s.detection.score));
        suggestions
    }

    /// 启动声明了 `service` 的插件，每个插件的数据目录为 `root/<插件 id>`；
    /// 单个插件启动失败只记录日志，不影响其他插件和应用启动
    pub fn start_services(&self, root: &Path) {
        for (id, plugin) in self.services() {
            let dir = root.join(&id);
            let started = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    plugin
                        .start_service(Arc::new(ServiceDir(dir)))
                        .map_err(|e| e.to_string())
                });
            if let Err(err) = started {
                log::warn!("plugin service {id} failed to start: {err}");
            }
        }
    }

    /// 应用退出前停止所有后台服务
    pub fn stop_services(&self) {
        for (_, plugin) in self.services() {
            plugin.stop_service();
        }
    }

    /// 当前插件的快照；检测剪贴板等耗时操作不持有锁
    fn snapshot(&self) -> Vec<(String, Arc<dyn ToolPlugin>)> {
        self.read()
            .iter()
            .map(|(id, p)| (id.clone(), p.clone()))
            .collect()
    }

    fn services(&self) -> Vec<(String, Arc<dyn ToolPlugin>)> {
        self.snapshot()
            .into_iter()
            .filter(|(_, p)| p.manifest().service)
            .collect()
    }

    /// 普通调用；声明为任务的函数必须通过 [`Self::run_task`] 运行
    pub fn call(&self, plugin_id: &str, function: &str, args: Value) -> AppResult<Value> {
        let (plugin, task) = self.resolve(plugin_id, function)?;
        if task {
            return Err(AppError::new("plugin.requires_task").with("function", function));
        }
        Ok(plugin.call(function, args)?)
    }

    /// 这次调用是否应记入处理历史：函数声明了 history 且插件不是 sensitive
    pub fn records_history(&self, plugin_id: &str, function: &str) -> bool {
        self.get(plugin_id).is_ok_and(|plugin| {
            let manifest = plugin.manifest();
            !manifest.sensitive && manifest.functions.get(function).is_some_and(|f| f.history)
        })
    }

    /// 插件是否为 sensitive（不保存任何输入）
    pub fn is_sensitive(&self, plugin_id: &str) -> bool {
        self.get(plugin_id)
            .is_ok_and(|plugin| plugin.manifest().sensitive)
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
