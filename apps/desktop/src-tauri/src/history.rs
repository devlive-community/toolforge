//! 历史记录命令：工具处理记录的写入、查询与清理。

use serde::Serialize;
use serde_json::{Value, json};
use tauri::State;
use tf_core::{AppResult, HistoryEntry, PluginRegistry, Store};
use tf_host::history::{ENABLED_KEY, enabled};

use crate::AppState;

const MAX_PAGE: usize = 200;

/// 插件调用成功后按需记入历史；规则与命令行、AI Agent 相同（见 tf_host::history）
pub fn record(
    store: &Store,
    registry: &PluginRegistry,
    plugin_id: &str,
    function: &str,
    args: &Value,
    result: &Value,
) {
    if let Ok(plugin) = registry.get(plugin_id) {
        tf_host::history::record(store, "app", plugin.manifest(), function, args, result);
    }
}

#[derive(Serialize)]
pub struct HistoryPage {
    entries: Vec<HistoryEntry>,
    total: usize,
}

#[tauri::command]
pub fn history_list(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
    offset: usize,
) -> AppResult<HistoryPage> {
    Ok(HistoryPage {
        entries: state
            .store
            .history_list(query.trim(), limit.min(MAX_PAGE), offset)?,
        total: state.store.history_count()?,
    })
}

#[tauri::command]
pub fn history_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.store.history_delete(id)
}

#[tauri::command]
pub fn history_clear(state: State<'_, AppState>, plugin_id: Option<String>) -> AppResult<usize> {
    state.store.history_clear(plugin_id.as_deref())
}

#[tauri::command]
pub fn history_settings_get(state: State<'_, AppState>) -> Value {
    json!({ "enabled": enabled(&state.store) })
}

#[tauri::command]
pub fn history_settings_set(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.store.kv_set(ENABLED_KEY, &json!(enabled))
}
