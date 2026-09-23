mod commands;
mod plugins;
mod tasks;
mod updater;
mod window;

use std::sync::Arc;

use tauri::Manager;
use tf_core::{PluginRegistry, Store, TaskManager};

/// 保留的任务记录与日志数量
const TASK_HISTORY: usize = 200;

pub struct AppState {
    pub store: Arc<Store>,
    pub plugins: Arc<PluginRegistry>,
    pub tasks: TaskManager,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = Arc::new(Store::open(&data_dir.join("toolforge.db"))?);
            let prefs = store.kv_get(commands::PREFS_KEY)?.unwrap_or_default();
            let tasks = TaskManager::new(
                store.clone(),
                data_dir.join("logs").join("tasks"),
                TASK_HISTORY,
            )?;
            window::create_main(app.handle(), &prefs)?;
            app.manage(AppState {
                store,
                plugins: Arc::new(plugins::builtin()),
                tasks,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::app_log,
            commands::prefs_get,
            commands::prefs_set,
            commands::favorites_list,
            commands::favorite_toggle,
            commands::recent_list,
            commands::recent_touch,
            commands::plugin_list,
            commands::plugin_call,
            commands::fs_read_text,
            commands::fs_write_text,
            tasks::task_start,
            tasks::task_cancel,
            tasks::task_list,
            tasks::task_logs,
            updater::update_check,
            updater::update_install,
            updater::app_restart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ToolForge");
}
