//! 官网「一键安装」：浏览器打开 toolforge://plugin/install/<插件 id> 时唤起应用，
//! 从官方插件索引取包、校验后直接安装，结果通过事件交给前端提示。
//!
//! 链接只在官网对 `channel: market` 的工具展示，因此这里只接受官方签名的包；
//! 未安装应用、版本过低等情况都由前端 toast 说明。

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tf_core::{AppError, AppResult};

use crate::installed::InstallInfo;
use crate::{AppState, market, version, window};

/// 一键安装结果事件：前端刷新插件列表并弹 toast（文案由前端 i18n 翻译）
pub const EXTERNAL_INSTALL: &str = "market://external-install";

/// 安装结果；installed 与 updated 带版本号，其余只需名称
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalInstall {
    pub id: String,
    /// installed / updated / uptodate / builtin / failed
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 语言 → 名称，供前端按当前语言显示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

impl ExternalInstall {
    fn done(id: &str, outcome: &str, info: &InstallInfo) -> Self {
        Self {
            id: id.to_owned(),
            outcome: outcome.to_owned(),
            version: Some(info.version.clone()),
            names: Some(json!(info.names)),
            error: None,
        }
    }
}

/// 解析 toolforge://plugin/install/<插件 id>，返回插件 id；其他链接一律忽略
fn install_id(url: &url::Url) -> Option<String> {
    if url.scheme() != "toolforge" || url.host_str() != Some("plugin") {
        return None;
    }
    let id = url.path().strip_prefix("/install/")?.split('/').next()?;
    let valid = !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-');
    valid.then(|| id.to_owned())
}

/// 官方索引中按 id 找条目
fn find_entry<'a>(index: &'a Value, id: &str) -> Option<&'a Value> {
    index
        .get("plugins")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
}

/// 注册深链监听；应用由链接启动时，回调会立即收到启动链接
pub fn init(app: &AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            match install_id(&url) {
                Some(id) => install(&handle, &id),
                None => log::warn!("ignored unsupported deep link: {url}"),
            }
        }
    });
}

/// 主窗口前置并后台安装，结果通过 EXTERNAL_INSTALL 事件通知前端
fn install(app: &AppHandle, id: &str) {
    window::show_main(app);
    let app = app.clone();
    let id = id.to_owned();
    std::thread::spawn(move || {
        let event = match install_from_index(&app, &id) {
            Ok(event) => event,
            Err(error) => {
                log::warn!("one-click install of {id} failed: {}", error.code);
                ExternalInstall {
                    id: id.clone(),
                    outcome: "failed".to_owned(),
                    version: None,
                    names: None,
                    error: Some(error),
                }
            }
        };
        let _ = app.emit(EXTERNAL_INSTALL, event);
    });
}

/// 从官方索引安装指定插件；已安装且不落后于索引时直接返回 up-to-date
fn install_from_index(app: &AppHandle, id: &str) -> AppResult<ExternalInstall> {
    let state = app.state::<AppState>();
    if state.builtin.contains(id) {
        return Ok(ExternalInstall {
            id: id.to_owned(),
            outcome: "builtin".to_owned(),
            version: None,
            names: None,
            error: None,
        });
    }

    let index = market::fetch_index()?;
    let entry = find_entry(&index, id)
        .ok_or_else(|| AppError::new("market.not_in_index").with("id", id))?;
    let version = entry
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let names = entry
        .get("names")
        .filter(|value| value.is_object())
        .cloned();

    let current = state
        .installed
        .list()
        .into_iter()
        .find(|info| info.id == id);
    if let Some(info) = current.as_ref()
        && version::at_least(&info.version, &version)
    {
        return Ok(ExternalInstall {
            id: id.to_owned(),
            outcome: "uptodate".to_owned(),
            version: Some(info.version.clone()),
            names: names.or_else(|| Some(json!(info.names))),
            error: None,
        });
    }

    if entry.get("signed").and_then(Value::as_bool) != Some(true) {
        return Err(AppError::new("market.unsigned").with("id", id));
    }
    if let Some(min) = entry.get("minAppVersion").and_then(Value::as_str) {
        let current_version = app.package_info().version.to_string();
        if !version::at_least(&current_version, min) {
            return Err(AppError::new("market.needs_app")
                .with("id", id)
                .with("version", min));
        }
    }

    let url = entry
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new("market.invalid_index"))?;
    let sha256 = entry
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new("market.invalid_index"))?;
    let bytes = market::fetch_package(url, sha256)?;
    let outcome = if current.is_some() {
        "updated"
    } else {
        "installed"
    };
    // 一键安装只接受官方签名；加载失败时 install_into 会撤销安装并报错
    let info = market::install_into(
        app,
        &state.installed,
        &state.plugins,
        &state.builtin,
        &bytes,
        Some(url),
        false,
    )?;
    Ok(ExternalInstall::done(id, outcome, &info))
}

#[cfg(test)]
#[path = "deeplink_test.rs"]
mod tests;
