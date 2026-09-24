//! 耗时任务命令：事件通过 Channel 推给发起方，同时广播摘要给全局任务中心。

use std::sync::Arc;

use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, State};
use tf_core::{AppResult, EventSink, LogLine, TaskEvent, TaskRecord};

use crate::AppState;

/// 全局任务事件（不含日志），任务中心订阅它
pub const TASK_EVENT: &str = "task://event";
const TASK_LIST_LIMIT: usize = 100;

pub struct TauriSink {
    channel: Channel<TaskEvent>,
    app: AppHandle,
}

impl TauriSink {
    pub fn new(channel: Channel<TaskEvent>, app: AppHandle) -> Self {
        Self { channel, app }
    }
}

impl EventSink for TauriSink {
    fn emit(&self, event: TaskEvent) {
        if !matches!(event, TaskEvent::Logs { .. }) {
            let _ = self.app.emit(TASK_EVENT, &event);
        }
        let _ = self.channel.send(event);
    }
}

#[tauri::command]
pub fn task_start(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
    function: String,
    args: Value,
    on_event: Channel<TaskEvent>,
) -> AppResult<String> {
    // 在启动线程前同步校验，错误直接返回给调用方
    state.plugins.ensure_task(&plugin_id, &function)?;
    let registry = state.plugins.clone();
    let sink = Arc::new(TauriSink::new(on_event, app));
    let (pid, func) = (plugin_id.clone(), function.clone());
    state.tasks.start(&plugin_id, &function, sink, move |ctx| {
        registry.run_task(&pid, &func, args, ctx)
    })
}

#[tauri::command]
pub fn task_cancel(state: State<'_, AppState>, task_id: String) -> bool {
    state.tasks.cancel(&task_id)
}

#[tauri::command]
pub fn task_list(state: State<'_, AppState>) -> AppResult<Vec<TaskRecord>> {
    state.tasks.list(TASK_LIST_LIMIT)
}

#[tauri::command]
pub async fn task_logs(state: State<'_, AppState>, task_id: String) -> AppResult<Vec<LogLine>> {
    state.tasks.logs(&task_id)
}
