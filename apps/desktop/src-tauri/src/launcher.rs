//! 快捷启动：全局快捷键把窗口带到前台并打开命令面板；
//! 开启后台运行时，关闭窗口只隐藏窗口，应用留在菜单栏 / 系统托盘中。

use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tf_core::{AppError, AppResult};

use crate::AppState;
use crate::commands::PREFS_KEY;
use crate::window::show_main;

/// 通知前端打开（或关闭）命令面板；载荷表示触发前窗口是否已在前台
pub const PALETTE: &str = "app://palette";
pub const DEFAULT_SHORTCUT: &str = "Alt+Space";
const TRAY_ID: &str = "main";
const TRAY_OPEN: &str = "tray.open";
const TRAY_PALETTE: &str = "tray.palette";
const TRAY_QUIT: &str = "tray.quit";

/// 当前已注册的全局快捷键，以及启动时注册失败的原因
#[derive(Debug, Clone, Default, Serialize)]
pub struct LauncherStatus {
    pub shortcut: Option<String>,
    pub error: Option<AppError>,
}

#[derive(Default)]
pub struct ActiveShortcut(Mutex<LauncherStatus>);

impl ActiveShortcut {
    pub fn status(&self) -> LauncherStatus {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// 偏好中的快捷键：未设置时使用默认值，显式为 null 表示关闭
pub fn shortcut_from(prefs: &Value) -> Option<String> {
    match prefs.get("globalShortcut") {
        None => Some(DEFAULT_SHORTCUT.to_owned()),
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        Some(_) => None,
    }
}

pub fn background_from(prefs: &Value) -> bool {
    prefs
        .get("runInBackground")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn prefs(app: &AppHandle) -> Value {
    app.try_state::<AppState>()
        .and_then(|state| state.store.kv_get(PREFS_KEY).ok().flatten())
        .unwrap_or_default()
}

/// 关闭窗口时是否只隐藏（后台运行）
pub fn keep_in_background(app: &AppHandle) -> bool {
    background_from(&prefs(app))
}

pub fn open_palette(app: &AppHandle) {
    let focused = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    show_main(app);
    let _ = app.emit(PALETTE, focused);
}

pub fn parse_shortcut(shortcut: &str) -> AppResult<Shortcut> {
    shortcut
        .parse()
        .map_err(|_| AppError::new("shortcut.invalid").with("shortcut", shortcut))
}

fn register(app: &AppHandle, shortcut: Option<&str>) -> AppResult<()> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    let Some(text) = shortcut else {
        return Ok(());
    };
    manager
        .on_shortcut(parse_shortcut(text)?, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                open_palette(app);
            }
        })
        .map_err(|e| {
            AppError::new("shortcut.unavailable")
                .with("shortcut", text)
                .with("detail", e.to_string())
        })
}

fn tray_labels(locale: &str) -> [&'static str; 3] {
    if locale.starts_with("zh") {
        ["打开 ToolForge", "命令面板", "退出 ToolForge"]
    } else {
        ["Open ToolForge", "Command Palette", "Quit ToolForge"]
    }
}

fn tray_menu(app: &AppHandle, locale: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let [open, palette, quit] = tray_labels(locale);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, TRAY_OPEN, open, true, None::<&str>)?,
            &MenuItem::with_id(app, TRAY_PALETTE, palette, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, TRAY_QUIT, quit, true, None::<&str>)?,
        ],
    )
}

fn on_tray_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        TRAY_OPEN => show_main(app),
        TRAY_PALETTE => open_palette(app),
        TRAY_QUIT if crate::lifecycle::request_quit(app) => app.exit(0),
        _ => {}
    }
}

/// 显示或移除托盘图标；已存在时只更新菜单语言
pub fn set_tray(app: &AppHandle, enabled: bool, locale: &str) -> tauri::Result<()> {
    if !enabled {
        let _ = app.remove_tray_by_id(TRAY_ID);
        return Ok(());
    }
    let menu = tray_menu(app, locale)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        return tray.set_menu(Some(menu));
    }
    // macOS 菜单栏使用单色模板图标，其他平台使用应用图标
    let icon = if cfg!(target_os = "macos") {
        Image::from_bytes(include_bytes!("../icons/tray-template.png"))?
    } else {
        app.default_window_icon()
            .cloned()
            .ok_or(tauri::Error::InvalidIcon(std::io::Error::other(
                "missing app icon",
            )))?
    };
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("ToolForge")
        .menu(&menu)
        // macOS 习惯左键弹出菜单；Windows / Linux 左键打开窗口，右键弹出菜单
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(on_tray_menu)
        .on_tray_icon_event(|tray, event| {
            if !cfg!(target_os = "macos")
                && let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 启动时按偏好注册快捷键并显示托盘
pub fn init(app: &AppHandle, prefs: &Value) {
    let shortcut = shortcut_from(prefs);
    let result = register(app, shortcut.as_deref());
    let state = app.state::<ActiveShortcut>();
    let mut status = state.0.lock().unwrap_or_else(|e| e.into_inner());
    match result {
        Ok(()) => status.shortcut = shortcut,
        Err(err) => {
            log::warn!("global shortcut not registered: {err}");
            status.error = Some(err);
        }
    }
    drop(status);
    let locale = prefs.get("locale").and_then(Value::as_str).unwrap_or("en");
    if let Err(err) = set_tray(app, background_from(prefs), locale) {
        log::warn!("tray icon not created: {err}");
    }
}

/// 设置全局快捷键；为空时关闭。注册失败时恢复之前的快捷键
#[tauri::command]
pub fn launcher_set_shortcut(
    app: AppHandle,
    active: State<'_, ActiveShortcut>,
    shortcut: Option<String>,
) -> AppResult<()> {
    let shortcut = shortcut.filter(|s| !s.trim().is_empty());
    let mut status = active.0.lock().unwrap_or_else(|e| e.into_inner());
    match register(&app, shortcut.as_deref()) {
        Ok(()) => {
            status.shortcut = shortcut;
            status.error = None;
            Ok(())
        }
        Err(err) => {
            let _ = register(&app, status.shortcut.as_deref());
            Err(err)
        }
    }
}

/// 快捷键状态：设置页据此提示启动时注册失败
#[tauri::command]
pub fn launcher_status(active: State<'_, ActiveShortcut>) -> LauncherStatus {
    active.status()
}

/// 开启或关闭后台运行（同时显示或移除托盘图标）
#[tauri::command]
pub fn launcher_set_background(app: AppHandle, enabled: bool, locale: String) -> AppResult<()> {
    set_tray(&app, enabled, &locale)
        .map_err(|e| AppError::new("app.unknown").with("detail", e.to_string()))
}

#[cfg(test)]
#[path = "launcher_test.rs"]
mod tests;
