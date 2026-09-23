mod commands;
mod plugins;
mod window;

use std::sync::Arc;

use tauri::Manager;
use tf_core::{PluginRegistry, Store};

pub struct AppState {
    pub store: Store,
    pub plugins: Arc<PluginRegistry>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = Store::open(&data_dir.join("toolforge.db"))?;
            let prefs = store.kv_get(commands::PREFS_KEY)?.unwrap_or_default();
            window::create_main(app.handle(), &prefs)?;
            app.manage(AppState {
                store,
                plugins: Arc::new(plugins::builtin()),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running ToolForge");
}
