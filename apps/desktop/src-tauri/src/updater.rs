//! 在线更新：检查、下载安装都在 Rust 侧完成，进度通过 Channel 实时推送给前端。

use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tf_core::markdown::{self, Block};
use tf_core::{AppError, AppResult};

/// 进度事件的最小推送间隔，避免高频 IPC
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// 最近一次检查到的更新，供安装时使用
#[derive(Default)]
pub struct PendingUpdate(Pending<Update>);

struct Pending<T> {
    state: Mutex<PendingState<T>>,
}

struct PendingState<T> {
    candidate: Option<T>,
    busy: bool,
}

impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(PendingState {
                candidate: None,
                busy: false,
            }),
        }
    }
}

/// A lease spans the asynchronous operation without holding a mutex guard.
/// Dropping a failed or cancelled operation releases it but keeps its candidate.
struct Operation<'a, T>(&'a Pending<T>);

impl<T> Pending<T> {
    fn begin_check(&self) -> AppResult<Operation<'_, T>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy {
            return Err(AppError::new("update.busy"));
        }
        state.busy = true;
        Ok(Operation(self))
    }
}

impl<T: Clone> Pending<T> {
    fn begin_install(&self) -> AppResult<(Operation<'_, T>, T)> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy {
            return Err(AppError::new("update.busy"));
        }
        let candidate = state
            .candidate
            .clone()
            .ok_or_else(|| AppError::new("update.none"))?;
        state.busy = true;
        Ok((Operation(self), candidate))
    }

    async fn install<F, Fut>(&self, install: F) -> AppResult<()>
    where
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = AppResult<()>>,
    {
        let (operation, update) = self.begin_install()?;
        install(update).await?;
        // Only successful installation consumes the update. Checks cannot
        // replace the candidate while this lease is active.
        operation.replace(None);
        Ok(())
    }
}

impl<T> Operation<'_, T> {
    fn replace(&self, candidate: Option<T>) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .candidate = candidate;
    }
}

impl<T> Drop for Operation<'_, T> {
    fn drop(&mut self) {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner()).busy = false;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    date: Option<String>,
    /// 发布说明（Markdown 已在 Rust 侧解析为结构化块）
    notes: Vec<Block>,
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
    let operation = pending.0.begin_check()?;
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
        notes: u.body.as_deref().map(markdown::parse).unwrap_or_default(),
    });
    operation.replace(update);
    Ok(info)
}

#[tauri::command]
pub async fn update_install(
    pending: State<'_, PendingUpdate>,
    on_event: Channel<UpdateEvent>,
) -> AppResult<()> {
    let events = on_event.clone();
    pending
        .0
        .install(|update| async move {
            let mut downloaded = 0u64;
            let mut last_emit: Option<Instant> = None;
            let progress = events.clone();
            let installing = events.clone();
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
                .map_err(|e| failed("update.install_failed", e))
        })
        .await?;
    let _ = on_event.send(UpdateEvent::Finished);
    Ok(())
}

#[cfg(test)]
#[path = "updater_test.rs"]
mod tests;

/// 安装完成后重启应用
#[tauri::command]
pub fn app_restart(app: AppHandle) {
    // 安装更新后的重启不需要退出确认
    app.state::<crate::lifecycle::QuitGuard>().confirm();
    crate::stop_services(&app);
    app.restart();
}
