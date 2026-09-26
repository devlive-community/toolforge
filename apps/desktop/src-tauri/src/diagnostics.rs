//! 诊断包：打包版本与系统信息、插件列表、最近的任务摘要与应用日志，方便反馈问题。
//! 不包含剪贴板、任务参数、文件内容等用户数据；偏好设置只导出白名单中的字段。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, State};
use tf_core::{AppError, AppResult, TaskRecord};
use zip::write::SimpleFileOptions;

use crate::AppState;
use crate::commands::{AppInfo, PREFS_KEY, app_info};
use crate::launcher::{ActiveShortcut, LauncherStatus};

const SAFE_PREFS: &[&str] = &[
    "theme",
    "locale",
    "autoUpdate",
    "confirmQuit",
    "clipboardSuggest",
    "globalShortcut",
    "runInBackground",
];
const TASKS: usize = 50;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    pub id: String,
    pub version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub generated_at: String,
    pub app: AppInfo,
    pub prefs: Map<String, Value>,
    pub launcher: LauncherStatus,
    pub plugins: Vec<PluginEntry>,
    pub tasks: Vec<TaskRecord>,
}

/// 只保留白名单中的偏好设置
pub fn safe_prefs(prefs: &Value) -> Map<String, Value> {
    SAFE_PREFS
        .iter()
        .filter_map(|key| Some((key.to_string(), prefs.get(*key)?.clone())))
        .collect()
}

fn io(err: impl std::fmt::Display) -> AppError {
    AppError::new("fs.io").with("detail", err.to_string())
}

/// 写出 zip：report.json 与 logs/ 下的日志文件
pub fn write_zip(path: &Path, report: &Report, logs: &[PathBuf]) -> AppResult<u64> {
    let file = std::fs::File::create(path).map_err(io)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("report.json", options).map_err(io)?;
    zip.write_all(&serde_json::to_vec_pretty(report).map_err(io)?)
        .map_err(io)?;
    for log in logs {
        let Some(name) = log.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read(log) else {
            continue;
        };
        zip.start_file(format!("logs/{name}"), options)
            .map_err(io)?;
        zip.write_all(&content).map_err(io)?;
    }
    zip.finish().map_err(io)?;
    std::fs::metadata(path).map(|m| m.len()).map_err(io)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    path: String,
    bytes: u64,
}

#[tauri::command]
pub async fn diagnostics_export(
    app: AppHandle,
    state: State<'_, AppState>,
    shortcut: State<'_, ActiveShortcut>,
    path: String,
) -> AppResult<Exported> {
    let prefs = state.store.kv_get(PREFS_KEY)?.unwrap_or_default();
    let report = Report {
        generated_at: tf_core::applog::format_time(tf_core::store::now_millis()),
        app: app_info(app),
        prefs: safe_prefs(&prefs),
        launcher: shortcut.status(),
        plugins: state
            .plugins
            .manifests()
            .into_iter()
            .map(|m| PluginEntry {
                id: m.id,
                version: m.version,
            })
            .collect(),
        tasks: state.store.tasks(TASKS)?,
    };
    // 日志文件从旧到新写入，阅读时顺序自然
    let mut logs = state.log.files();
    logs.reverse();
    log::info!("exporting diagnostics");
    state.log.flush_now();
    let target = PathBuf::from(&path);
    let bytes = tauri::async_runtime::spawn_blocking(move || write_zip(&target, &report, &logs))
        .await
        .map_err(io)??;
    Ok(Exported { path, bytes })
}

#[cfg(test)]
#[path = "diagnostics_test.rs"]
mod tests;
