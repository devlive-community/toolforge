mod commands;
mod lifecycle;
mod menu;
mod plugins;
mod resources;
mod tasks;
mod updater;
mod window;

use std::sync::Arc;

use tauri::{Manager, RunEvent};
use tf_core::{PluginRegistry, Resources, Store, TaskManager};

/// 保留的任务记录与日志数量
const TASK_HISTORY: usize = 200;

pub struct AppState {
    pub store: Arc<Store>,
    pub plugins: Arc<PluginRegistry>,
    pub tasks: TaskManager,
    pub resources: Arc<Resources>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .manage(resources::ActiveDownloads::default())
        .manage(lifecycle::QuitGuard::default())
        .on_menu_event(menu::on_event)
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = Arc::new(Store::open(&data_dir.join("toolforge.db"))?);
            let prefs = store.kv_get(commands::PREFS_KEY)?.unwrap_or_default();
            let resources = Arc::new(Resources::new(data_dir.join("resources")));
            let tasks = TaskManager::new(
                store.clone(),
                data_dir.join("logs").join("tasks"),
                TASK_HISTORY,
            )?
            .with_resources(resources.clone());
            window::create_main(app.handle(), &prefs)?;
            #[cfg(target_os = "macos")]
            {
                let locale = prefs.get("locale").and_then(|v| v.as_str()).unwrap_or("en");
                app.set_menu(menu::build(app.handle(), locale)?)?;
            }
            app.manage(AppState {
                store,
                plugins: Arc::new(plugins::builtin()),
                tasks,
                resources,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::app_log,
            commands::app_open_url,
            lifecycle::app_close_ack,
            lifecycle::app_quit,
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
            commands::plugin_state_get,
            commands::plugin_state_set,
            commands::fs_read_text,
            commands::fs_write_text,
            tasks::task_start,
            tasks::task_cancel,
            tasks::task_list,
            tasks::task_logs,
            resources::resource_list,
            resources::resource_download,
            resources::resource_delete,
            updater::update_check,
            updater::update_install,
            updater::app_restart,
        ])
        .build(tauri::generate_context!())
        .expect("error while building ToolForge")
        .run(|app, event| {
            // ⌘Q、菜单退出等（code 为空）先交给前端确认；app.exit() 带退出码直接放行
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = event
                && !lifecycle::request_quit(app)
            {
                api.prevent_exit();
            }
        });
}
