//! 历史记录命令：工具处理记录的写入、查询与清理。

use serde::Serialize;
use serde_json::{Value, json};
use tauri::State;
use tf_core::{AppResult, HistoryEntry, PluginRegistry, Store, history, store::now_millis};

use crate::AppState;

const ENABLED_KEY: &str = "history.enabled";
const MAX_PAGE: usize = 200;

pub fn enabled(store: &Store) -> bool {
    store
        .kv_get(ENABLED_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// 插件调用成功后按需记入历史；失败只写日志，不影响调用结果
pub fn record(
    store: &Store,
    registry: &PluginRegistry,
    plugin_id: &str,
    function: &str,
    args: &Value,
    result: &Value,
) {
    if !registry.records_history(plugin_id, function) || !enabled(store) {
        return;
    }
    let Some(input) = history::primary_text(args) else {
        return;
    };
    let output = history::output_preview(result);
    let outcome = store
        .history_record(plugin_id, function, &input, output.as_deref(), now_millis())
        .and_then(|_| store.history_prune(history::KEEP));
    if let Err(err) = outcome {
        log::warn!("history record failed: {}", err.code);
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
