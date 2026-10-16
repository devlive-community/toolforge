//! 插件市场命令：安装、更新、卸载在线插件，读取插件索引，并通过 tfplugin:// 提供插件界面文件。

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::Value;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State, UriSchemeContext, Wry};
use tf_core::{AppError, AppResult, PluginRegistry, resources};

use crate::AppState;
use crate::installed::{Assets, InstallInfo, Installed};

/// 插件列表变化（安装、卸载、启动时加载完成），前端据此刷新
pub const PLUGINS_CHANGED: &str = "plugins://changed";
/// 官方插件索引：插件独立于应用版本发布到固定的 plugins 发布（plugins.yml），修复无需等应用发版；
/// 可用 TOOLFORGE_PLUGIN_INDEX 覆盖（调试用）
const INDEX_URL: &str =
    "https://github.com/devlive-community/toolforge/releases/download/plugins/registry.json";
const MAX_INDEX: u64 = 2 * 1024 * 1024;
const MAX_PACKAGE: u64 = tf_package::MAX_PACKAGE as u64;

/// 启动后在后台加载已安装的插件，避免组件编译拖慢窗口出现
pub fn load_installed(
    app: &AppHandle,
    installed: Arc<Installed>,
    registry: Arc<PluginRegistry>,
    builtin: Arc<HashSet<String>>,
) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut loaded = 0;
        for info in installed.list() {
            if builtin.contains(&info.id) {
                log::warn!(
                    "installed plugin {} shadows a built-in plugin; skipped",
                    info.id
                );
                continue;
            }
            match installed.load(&info.id) {
                Ok(plugin) => {
                    registry.register(plugin);
                    loaded += 1;
                }
                Err(err) => log::warn!("installed plugin {} failed to load: {}", info.id, err.code),
            }
        }
        if loaded > 0 {
            let _ = app.emit(PLUGINS_CHANGED, ());
        }
    });
}

/// 安装并立即启用；加载失败时撤销安装
fn install(
    app: &AppHandle,
    state: &AppState,
    bytes: &[u8],
    url: Option<&str>,
    allow_unsigned: bool,
) -> AppResult<InstallInfo> {
    let builtin = state.builtin.clone();
    let (info, _) = state
        .installed
        .install(bytes, url, allow_unsigned, |id| builtin.contains(id))?;
    match state.installed.load(&info.id) {
        Ok(plugin) => {
            state.plugins.unregister(&info.id);
            state.plugins.register(plugin);
        }
        Err(err) => {
            let _ = state.installed.uninstall(&info.id);
            state.plugins.unregister(&info.id);
            return Err(err);
        }
    }
    log::info!(
        "installed plugin {} {} ({})",
        info.id,
        info.version,
        info.trust
    );
    let _ = app.emit(PLUGINS_CHANGED, ());
    Ok(info)
}

#[tauri::command]
pub fn plugin_installed(state: State<'_, AppState>) -> Vec<InstallInfo> {
    state.installed.list()
}

#[tauri::command]
pub async fn plugin_install_file(
    app: AppHandle,
    path: String,
    allow_unsigned: bool,
) -> AppResult<InstallInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let size = std::fs::metadata(&path)?.len();
        if size > MAX_PACKAGE {
            return Err(AppError::new("package.too_large").with("mb", 64));
        }
        let bytes = std::fs::read(&path)?;
        install(&app, &app.state::<AppState>(), &bytes, None, allow_unsigned)
    })
    .await
    .map_err(|e| AppError::new("plugin.crashed").with("detail", e.to_string()))?
}

#[tauri::command]
pub async fn plugin_install_url(
    app: AppHandle,
    url: String,
    sha256: String,
    allow_unsigned: bool,
) -> AppResult<InstallInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = resources::fetch_bytes(&url, MAX_PACKAGE)?;
        let actual = resources::to_hex(&sha2_digest(&bytes));
        if !actual.eq_ignore_ascii_case(sha256.trim()) {
            return Err(AppError::new("package.checksum_mismatch")
                .with("expected", sha256)
                .with("actual", actual));
        }
        install(
            &app,
            &app.state::<AppState>(),
            &bytes,
            Some(&url),
            allow_unsigned,
        )
    })
    .await
    .map_err(|e| AppError::new("plugin.crashed").with("detail", e.to_string()))?
}

fn sha2_digest(bytes: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).to_vec()
}

#[tauri::command]
pub fn plugin_uninstall(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
) -> AppResult<()> {
    state.installed.uninstall(&plugin_id)?;
    state.plugins.unregister(&plugin_id);
    let _ = app.emit(PLUGINS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn plugin_assets(state: State<'_, AppState>, plugin_id: String) -> AppResult<Assets> {
    state.installed.assets(&plugin_id)
}

/// 读取官方插件索引（JSON 数组），由前端与已安装版本比较
#[tauri::command]
pub async fn market_index() -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(|| {
        let url = std::env::var("TOOLFORGE_PLUGIN_INDEX").unwrap_or_else(|_| INDEX_URL.to_owned());
        let bytes = resources::fetch_bytes(&url, MAX_INDEX)?;
        let index: Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::new("market.invalid_index").with("detail", e.to_string()))?;
        if !index.get("plugins").is_some_and(Value::is_array) {
            return Err(AppError::new("market.invalid_index"));
        }
        Ok(index)
    })
    .await
    .map_err(|e| AppError::new("plugin.crashed").with("detail", e.to_string()))?
}

fn respond(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        // 插件界面以 ES module 方式跨源加载，需要 CORS
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-cache")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// tfplugin://localhost/<插件 id>/<文件>（Windows 上为 http://tfplugin.localhost/...）
pub fn protocol(ctx: UriSchemeContext<'_, Wry>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = request.uri().path().trim_start_matches('/');
    let Some((id, file)) = path.split_once('/') else {
        return respond(StatusCode::NOT_FOUND, "text/plain", Vec::new());
    };
    let mime = match file.rsplit('.').next() {
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    match ctx
        .app_handle()
        .state::<AppState>()
        .installed
        .file(id, file)
    {
        Ok(body) => respond(StatusCode::OK, mime, body),
        Err(_) => respond(StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}
