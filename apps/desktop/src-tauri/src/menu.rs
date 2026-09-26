//! macOS 应用菜单：「关于」与「设置」打开应用内页面，不使用系统默认的关于面板。
//! 其他平台使用无边框窗口，不显示菜单栏。

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Runtime};

/// 通知前端切换页面，载荷为视图名（about / settings）
pub const NAVIGATE: &str = "app://navigate";
const ABOUT: &str = "about";
const SETTINGS: &str = "settings";

struct Labels {
    about: &'static str,
    settings: &'static str,
    services: &'static str,
    hide: &'static str,
    hide_others: &'static str,
    show_all: &'static str,
    quit: &'static str,
    edit: &'static str,
    undo: &'static str,
    redo: &'static str,
    cut: &'static str,
    copy: &'static str,
    paste: &'static str,
    select_all: &'static str,
    window: &'static str,
    minimize: &'static str,
    zoom: &'static str,
    fullscreen: &'static str,
    close: &'static str,
}

fn labels(locale: &str) -> Labels {
    if locale.starts_with("zh") {
        Labels {
            about: "关于 ToolForge",
            settings: "设置…",
            services: "服务",
            hide: "隐藏 ToolForge",
            hide_others: "隐藏其他",
            show_all: "全部显示",
            quit: "退出 ToolForge",
            edit: "编辑",
            undo: "撤销",
            redo: "重做",
            cut: "剪切",
            copy: "拷贝",
            paste: "粘贴",
            select_all: "全选",
            window: "窗口",
            minimize: "最小化",
            zoom: "缩放",
            fullscreen: "进入全屏幕",
            close: "关闭窗口",
        }
    } else {
        Labels {
            about: "About ToolForge",
            settings: "Settings…",
            services: "Services",
            hide: "Hide ToolForge",
            hide_others: "Hide Others",
            show_all: "Show All",
            quit: "Quit ToolForge",
            edit: "Edit",
            undo: "Undo",
            redo: "Redo",
            cut: "Cut",
            copy: "Copy",
            paste: "Paste",
            select_all: "Select All",
            window: "Window",
            minimize: "Minimize",
            zoom: "Zoom",
            fullscreen: "Enter Full Screen",
            close: "Close Window",
        }
    }
}

pub fn build<R: Runtime>(app: &AppHandle<R>, locale: &str) -> tauri::Result<Menu<R>> {
    let l = labels(locale);
    let sep = || PredefinedMenuItem::separator(app);
    let app_menu = Submenu::with_items(
        app,
        "ToolForge",
        true,
        &[
            &MenuItem::with_id(app, ABOUT, l.about, true, None::<&str>)?,
            &sep()?,
            &MenuItem::with_id(app, SETTINGS, l.settings, true, Some("CmdOrCtrl+,"))?,
            &sep()?,
            &PredefinedMenuItem::services(app, Some(l.services))?,
            &sep()?,
            &PredefinedMenuItem::hide(app, Some(l.hide))?,
            &PredefinedMenuItem::hide_others(app, Some(l.hide_others))?,
            &PredefinedMenuItem::show_all(app, Some(l.show_all))?,
            &sep()?,
            // 系统退出会触发 ExitRequested，由 lifecycle 弹出确认
            &PredefinedMenuItem::quit(app, Some(l.quit))?,
        ],
    )?;
    // 编辑菜单提供 ⌘C / ⌘V 等快捷键，输入框依赖它们
    let edit = Submenu::with_items(
        app,
        l.edit,
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(l.undo))?,
            &PredefinedMenuItem::redo(app, Some(l.redo))?,
            &sep()?,
            &PredefinedMenuItem::cut(app, Some(l.cut))?,
            &PredefinedMenuItem::copy(app, Some(l.copy))?,
            &PredefinedMenuItem::paste(app, Some(l.paste))?,
            &PredefinedMenuItem::select_all(app, Some(l.select_all))?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        l.window,
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some(l.minimize))?,
            &PredefinedMenuItem::maximize(app, Some(l.zoom))?,
            &PredefinedMenuItem::fullscreen(app, Some(l.fullscreen))?,
            &sep()?,
            &PredefinedMenuItem::close_window(app, Some(l.close))?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit, &window])
}

pub fn on_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let view = match event.id().as_ref() {
        ABOUT => ABOUT,
        SETTINGS => SETTINGS,
        _ => return,
    };
    crate::window::show_main(app);
    let _ = app.emit(NAVIGATE, view);
}

/// 语言切换后重建菜单
#[tauri::command]
pub fn app_menu_locale(app: AppHandle, locale: String) -> tf_core::AppResult<()> {
    if cfg!(target_os = "macos") {
        let menu = build(&app, &locale)
            .map_err(|e| tf_core::AppError::new("app.unknown").with("detail", e.to_string()))?;
        app.set_menu(menu)
            .map_err(|e| tf_core::AppError::new("app.unknown").with("detail", e.to_string()))?;
    }
    // 托盘菜单跟随界面语言
    if app.tray_by_id("main").is_some() {
        crate::launcher::set_tray(&app, true, &locale)
            .map_err(|e| tf_core::AppError::new("app.unknown").with("detail", e.to_string()))?;
    }
    Ok(())
}
