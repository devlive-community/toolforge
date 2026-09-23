use serde::Serialize;
use serde_json::Value;
use tauri::State;
use tf_core::{AppError, AppResult};
use tf_plugin_api::Manifest;

use crate::AppState;

pub const PREFS_KEY: &str = "prefs";
const RECENT_LIMIT: usize = 12;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    os: &'static str,
    arch: &'static str,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

/// 前端日志转发到终端（开发期排查 WebView 内的错误）
#[tauri::command]
pub fn app_log(level: String, message: String) {
    eprintln!("[webview:{level}] {message}");
}

#[tauri::command]
pub fn prefs_get(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(state.store.kv_get(PREFS_KEY)?.unwrap_or_default())
}

#[tauri::command]
pub fn prefs_set(state: State<'_, AppState>, prefs: Value) -> AppResult<()> {
    state.store.kv_set(PREFS_KEY, &prefs)
}

#[tauri::command]
pub fn favorites_list(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.store.favorites()
}

#[tauri::command]
pub fn favorite_toggle(state: State<'_, AppState>, plugin_id: String) -> AppResult<bool> {
    state.store.toggle_favorite(&plugin_id)
}

#[tauri::command]
pub fn recent_list(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.store.recent(RECENT_LIMIT)
}

#[tauri::command]
pub fn recent_touch(state: State<'_, AppState>, plugin_id: String) -> AppResult<()> {
    state.store.touch_recent(&plugin_id)
}

#[tauri::command]
pub fn plugin_list(state: State<'_, AppState>) -> Vec<Manifest> {
    state.plugins.manifests()
}

/// 调用插件函数。在阻塞线程池中执行，避免数据处理阻塞 IPC 主循环。
#[tauri::command]
pub async fn plugin_call(
    state: State<'_, AppState>,
    plugin_id: String,
    function: String,
    args: Value,
) -> AppResult<Value> {
    let registry = state.plugins.clone();
    tauri::async_runtime::spawn_blocking(move || registry.call(&plugin_id, &function, args))
        .await
        .map_err(|e| AppError::new("plugin.crashed").with("detail", e.to_string()))?
}

/// 编辑器可承载的文本文件上限；更大的文件需走流式处理
const MAX_TEXT_FILE: u64 = 20 * 1024 * 1024;

/// 读取用户通过系统对话框选择或拖入的文本文件
#[tauri::command]
pub async fn fs_read_text(path: String) -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let size = std::fs::metadata(&path)?.len();
        if size > MAX_TEXT_FILE {
            return Err(AppError::new("fs.too_large").with("limit", "20 MB"));
        }
        let bytes = std::fs::read(&path)?;
        String::from_utf8(bytes).map_err(|_| AppError::new("fs.not_utf8"))
    })
    .await
    .map_err(|e| AppError::new("fs.io").with("detail", e.to_string()))?
}

/// 写入用户通过保存对话框选择的文件
#[tauri::command]
pub async fn fs_write_text(path: String, text: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || Ok(std::fs::write(&path, text)?))
        .await
        .map_err(|e| AppError::new("fs.io").with("detail", e.to_string()))?
}
