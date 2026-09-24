//! 插件资源（模型等）：查询状态、以任务方式下载（实时进度与日志）、删除。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tf_core::{AppError, AppResult, ResourceStatus, TaskEvent};
use tf_plugin_api::ResourceSpec;

use crate::AppState;
use crate::tasks::TauriSink;

/// 下载任务在任务中心显示的函数名
pub const DOWNLOAD_FUNCTION: &str = "download_resource";

type Key = (String, String);

/// 正在下载的资源，避免同一资源并发下载
#[derive(Default)]
pub struct ActiveDownloads(Arc<Mutex<HashSet<Key>>>);

impl ActiveDownloads {
    fn claim(&self, key: Key) -> Option<Claim> {
        let mut set = self.0.lock().unwrap_or_else(|e| e.into_inner());
        set.insert(key.clone()).then(|| Claim {
            set: self.0.clone(),
            key,
        })
    }

    fn contains(&self, key: &Key) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(key)
    }
}

/// 下载占用标记；任务结束（包括失败、panic 或未能启动）时自动释放
struct Claim {
    set: Arc<Mutex<HashSet<Key>>>,
    key: Key,
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.set
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.key);
    }
}

fn spec(state: &AppState, plugin_id: &str, resource_id: &str) -> AppResult<ResourceSpec> {
    state
        .plugins
        .get(plugin_id)?
        .manifest()
        .resources
        .iter()
        .find(|r| r.id == resource_id)
        .cloned()
        .ok_or_else(|| {
            AppError::new("resource.unknown")
                .with("plugin", plugin_id)
                .with("id", resource_id)
        })
}

#[tauri::command]
pub fn resource_list(
    state: State<'_, AppState>,
    plugin_id: String,
) -> AppResult<Vec<ResourceStatus>> {
    let plugin = state.plugins.get(&plugin_id)?;
    Ok(state
        .resources
        .status(&plugin_id, &plugin.manifest().resources))
}

#[tauri::command]
pub fn resource_download(
    app: AppHandle,
    state: State<'_, AppState>,
    active: State<'_, ActiveDownloads>,
    plugin_id: String,
    resource_id: String,
    on_event: Channel<TaskEvent>,
) -> AppResult<String> {
    let spec = spec(&state, &plugin_id, &resource_id)?;
    let claim = active
        .claim((plugin_id.clone(), resource_id.clone()))
        .ok_or_else(|| AppError::new("resource.busy").with("id", resource_id.as_str()))?;
    let resources = state.resources.clone();
    let sink = Arc::new(TauriSink::new(on_event, app));
    let pid = plugin_id.clone();
    state
        .tasks
        .start(&plugin_id, DOWNLOAD_FUNCTION, sink, move |ctx| {
            let _claim = claim;
            resources
                .download(&pid, &spec, ctx)
                .map(|_| json!({ "id": spec.id }))
                .map_err(AppError::from)
        })
}

#[tauri::command]
pub fn resource_delete(
    state: State<'_, AppState>,
    active: State<'_, ActiveDownloads>,
    plugin_id: String,
    resource_id: String,
) -> AppResult<()> {
    spec(&state, &plugin_id, &resource_id)?;
    if active.contains(&(plugin_id.clone(), resource_id.clone())) {
        return Err(AppError::new("resource.busy").with("id", resource_id));
    }
    state.resources.remove(&plugin_id, &resource_id)
}
