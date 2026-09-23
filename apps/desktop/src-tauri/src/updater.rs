//! 在线更新：检查、下载安装都在 Rust 侧完成，进度通过 Channel 实时推送给前端。

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tf_core::{AppError, AppResult};

/// 进度事件的最小推送间隔，避免高频 IPC
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// 最近一次检查到的更新，供安装时使用
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    date: Option<String>,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UpdateEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        downloaded: u64,
        total: Option<u64>,
    },
    Installing,
    Finished,
}

fn failed(code: &str, err: impl ToString) -> AppError {
    AppError::new(code).with("detail", err.to_string())
}

#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> AppResult<Option<UpdateInfo>> {
    let update = app
        .updater()
        .map_err(|e| failed("update.check_failed", e))?
        .check()
        .await
        .map_err(|e| failed("update.check_failed", e))?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        date: u.date.map(|d| d.to_string()),
        notes: u.body.clone(),
    });
    *pending.0.lock().unwrap_or_else(|e| e.into_inner()) = update;
    Ok(info)
}

#[tauri::command]
pub async fn update_install(
    pending: State<'_, PendingUpdate>,
    on_event: Channel<UpdateEvent>,
) -> AppResult<()> {
    let update = pending
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or_else(|| AppError::new("update.none"))?;

    let mut downloaded = 0u64;
    let mut last_emit: Option<Instant> = None;
    let progress = on_event.clone();
    let installing = on_event.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let due = last_emit.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL);
                if due || total == Some(downloaded) {
                    last_emit = Some(Instant::now());
                    let _ = progress.send(UpdateEvent::Progress { downloaded, total });
                }
            },
            move || {
                let _ = installing.send(UpdateEvent::Installing);
            },
        )
        .await
        .map_err(|e| failed("update.install_failed", e))?;
    let _ = on_event.send(UpdateEvent::Finished);
    Ok(())
}

/// 安装完成后重启应用
#[tauri::command]
pub fn app_restart(app: AppHandle) {
    app.restart();
}
