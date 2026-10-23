mod agent;
mod cli_tools;
mod commands;
mod deeplink;
mod diagnostics;
mod history;
mod launcher;
mod lifecycle;
mod market;
mod menu;
mod plugin_download;
mod plugin_stats;
mod plugins;
mod resources;
mod tasks;
mod updater;
mod window;

use std::collections::HashSet;
use std::sync::Arc;

use tauri::{Manager, RunEvent};
use tf_core::{AppLog, PluginRegistry, Resources, Store, TaskManager};
pub use tf_host::{installed, version};

/// 应用日志单个文件上限与保留的文件数
const LOG_BYTES: u64 = 2 * 1024 * 1024;
const LOG_FILES: usize = 3;

pub struct AppState {
    pub store: Arc<Store>,
    pub plugins: Arc<PluginRegistry>,
    pub tasks: TaskManager,
    pub resources: Arc<Resources>,
    pub log: &'static AppLog,
    /// 在线安装的插件
    pub installed: Arc<installed::Installed>,
    /// 内置插件 id：在线插件不能与之重名
    pub builtin: Arc<HashSet<String>>,
}

/// 停止插件后台服务；重启前也要调用，因为重启不会触发 RunEvent::Exit
pub fn stop_services(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.plugins.stop_services();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        // 必须最先注册：二次启动（含浏览器唤起的一键安装）时把事件转发给已有实例；
        // deep-link 特性会让插件自己处理 argv 里的链接
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show_main(app);
        }));
    }
    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .manage(resources::ActiveDownloads::default())
        .manage(lifecycle::QuitGuard::default())
        .manage(launcher::ActiveShortcut::default())
        .on_menu_event(menu::on_event)
        .register_uri_scheme_protocol("tfplugin", market::protocol)
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let log = AppLog::new(data_dir.join("logs"), LOG_BYTES, LOG_FILES).install();
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                log::error!("panic: {info}");
                previous(info);
            }));
            log::info!(
                "ToolForge {} starting on {} {}",
                app.package_info().version,
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            let store = Arc::new(Store::open(&data_dir.join("toolforge.db"))?);
            let prefs = store.kv_get(commands::PREFS_KEY)?.unwrap_or_default();
            let resources = Arc::new(Resources::new(data_dir.join("resources")));
            let tasks = TaskManager::new(store.clone(), data_dir.join("logs").join("tasks"))?
                .with_resources(resources.clone());
            window::create_main(app.handle(), &prefs)?;
            #[cfg(target_os = "macos")]
            {
                let locale = prefs.get("locale").and_then(|v| v.as_str()).unwrap_or("en");
                app.set_menu(menu::build(app.handle(), locale)?)?;
            }
            let plugins = Arc::new(plugins::builtin());
            plugins.start_services(&data_dir.join("plugins"));
            let builtin: Arc<HashSet<String>> =
                Arc::new(plugins.manifests().into_iter().map(|m| m.id).collect());
            let installed = Arc::new(installed::Installed::new(
                data_dir.join("installed"),
                app.package_info().version.to_string(),
            ));
            market::load_installed(
                app.handle(),
                installed.clone(),
                plugins.clone(),
                builtin.clone(),
                store.clone(),
            );
            plugin_stats::init(store.clone());
            app.manage(AppState {
                store,
                plugins,
                tasks,
                resources,
                log,
                installed,
                builtin,
            });
            // Windows / Linux 在运行时注册 toolforge:// scheme（macOS 由构建时 Info.plist 声明）
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(err) = app.deep_link().register_all() {
                    log::warn!("failed to register deep link schemes: {err}");
                }
            }
            // 官网 toolforge:// 一键安装；须在 AppState 之后注册，处理链接时要用到插件状态
            deeplink::init(app.handle());
            launcher::init(app.handle(), &prefs);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            agent::agent_overview,
            agent::agent_set_enabled,
            agent::agent_add_path,
            agent::agent_remove_path,
            agent::interface_support,
            agent::interface_set_plugin,
            agent::interface_set_function,
            agent::plugin_interface_config,
            agent::plugin_interfaces,
            cli_tools::cli_status,
            cli_tools::cli_install,
            cli_tools::cli_uninstall,
            commands::app_info,
            commands::app_log,
            commands::app_open_url,
            lifecycle::app_close_ack,
            lifecycle::app_quit,
            launcher::launcher_set_shortcut,
            launcher::launcher_set_background,
            launcher::launcher_status,
            diagnostics::diagnostics_export,
            menu::app_menu_locale,
            commands::app_reveal_path,
            commands::prefs_get,
            commands::prefs_set,
            commands::favorites_list,
            commands::favorite_toggle,
            commands::recent_list,
            commands::recent_touch,
            commands::plugin_list,
            commands::plugin_call,
            commands::clipboard_suggestions,
            commands::plugin_state_get,
            commands::plugin_state_set,
            commands::fs_read_text,
            commands::fs_write_text,
            tasks::task_start,
            tasks::task_cancel,
            tasks::task_list,
            tasks::task_logs,
            tasks::task_rerun,
            tasks::task_clear,
            history::history_list,
            history::history_delete,
            history::history_clear,
            history::history_settings_get,
            history::history_settings_set,
            market::plugin_installed,
            market::plugin_install_file,
            market::plugin_install_url,
            market::plugin_uninstall,
            market::plugin_assets,
            market::market_index,
            plugin_stats::plugin_download_stats,
            resources::resource_list,
            resources::resource_download,
            resources::resource_delete,
            updater::update_check,
            updater::update_install,
            updater::app_restart,
        ])
        .build(tauri::generate_context!())
        .expect("error while building ToolForge")
        .run(|app, event| match event {
            // ⌘Q、菜单退出等（code 为空）先交给前端确认；app.exit() 带退出码直接放行
            RunEvent::ExitRequested {
                code: None, api, ..
            } if !lifecycle::request_quit(app) => api.prevent_exit(),
            // 退出前让后台服务把数据落盘
            RunEvent::Exit => stop_services(app),
            // macOS 点击 Dock 图标时重新显示隐藏的窗口
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => window::show_main(app),
            _ => {}
        });
}
