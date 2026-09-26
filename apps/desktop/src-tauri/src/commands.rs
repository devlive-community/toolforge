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
    tauri_version: &'static str,
    webview_version: Option<String>,
    data_dir: Option<String>,
    log_dir: Option<String>,
}

#[cfg(test)]
impl AppInfo {
    pub fn test_value() -> Self {
        Self {
            version: "0.0.0".into(),
            os: "test",
            arch: "test",
            tauri_version: "2",
            webview_version: None,
            data_dir: None,
            log_dir: None,
        }
    }
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle) -> AppInfo {
    use tauri::Manager;
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        tauri_version: tauri::VERSION,
        webview_version: tauri::webview_version().ok(),
        data_dir: app
            .path()
            .app_data_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned()),
        log_dir: app
            .path()
            .app_data_dir()
            .ok()
            .map(|p| p.join("logs").to_string_lossy().into_owned()),
    }
}

/// 用系统浏览器打开外部链接；只允许 http(s)，避免借此打开本地文件或任意协议
#[tauri::command]
pub fn app_open_url(app: tauri::AppHandle, url: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(AppError::new("app.invalid_url").with("url", url));
    }
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| AppError::new("app.open_failed").with("detail", e.to_string()))
}

/// 在系统文件管理器中显示文件
#[tauri::command]
pub fn app_reveal_path(app: tauri::AppHandle, path: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !std::path::Path::new(&path).exists() {
        return Err(AppError::new("fs.not_found").with("path", path));
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| AppError::new("app.open_failed").with("detail", e.to_string()))
}

/// 前端日志写入应用日志（WebView 内的错误等）
#[tauri::command]
pub fn app_log(level: String, message: String) {
    let level = match level.as_str() {
        "error" => log::Level::Error,
        "warn" => log::Level::Warn,
        _ => log::Level::Info,
    };
    log::log!(target: "webview", level, "{message}");
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

/// 读取插件私有状态（例如编辑器草稿），不存在时返回 null
#[tauri::command]
pub fn plugin_state_get(
    state: State<'_, AppState>,
    plugin_id: String,
    key: String,
) -> AppResult<Option<Value>> {
    state.plugins.get(&plugin_id)?;
    state.store.plugin_state_get(&plugin_id, &key)
}

/// 写入插件私有状态；值为 null 时删除
#[tauri::command]
pub fn plugin_state_set(
    state: State<'_, AppState>,
    plugin_id: String,
    key: String,
    value: Value,
) -> AppResult<()> {
    state.plugins.get(&plugin_id)?;
    state.store.plugin_state_set(&plugin_id, &key, &value)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSuggestions {
    /// 剪贴板文本；没有推荐时为空，避免无谓地把内容传给界面
    text: Option<String>,
    preview: String,
    /// 剪贴板中是图片时的尺寸（宽、高）；图片内容不传给界面，由插件自行读取
    image: Option<(u32, u32)>,
    suggestions: Vec<tf_core::Suggestion>,
}

/// 读取剪贴板文本并询问各插件能否处理，用于命令面板的「来自剪贴板」推荐
#[tauri::command]
pub async fn clipboard_suggestions(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ClipboardSuggestions> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let text = app.clipboard().read_text().unwrap_or_default();
    // 没有文本时再看是否为图片（例如截图）
    let image = text
        .trim()
        .is_empty()
        .then(|| app.clipboard().read_image().ok())
        .flatten()
        .map(|image| (image.width(), image.height()));
    let registry = state.plugins.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some((width, height)) = image {
            return ClipboardSuggestions {
                text: None,
                preview: String::new(),
                image: Some((width, height)),
                suggestions: registry.detect_image(width, height),
            };
        }
        let suggestions = registry.detect(&text);
        ClipboardSuggestions {
            preview: tf_core::preview(&text, 80),
            text: (!suggestions.is_empty()).then(|| text.trim().to_owned()),
            image: None,
            suggestions,
        }
    })
    .await
    .map_err(|e| AppError::new("plugin.crashed").with("detail", e.to_string()))
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
