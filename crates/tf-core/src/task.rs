//! 耗时任务：独立线程运行，日志 / 进度 / 阶段实时推送，支持取消，日志落盘，记录存入 SQLite。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};

use crate::store::{TaskRecord, now_millis};
use crate::{AppError, AppResult, Resources, Store};

/// 日志批量推送的时间窗口与条数上限
const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const FLUSH_LINES: usize = 200;
/// 进度事件的最小推送间隔
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);
/// 读取历史日志时最多返回的行数
const MAX_LOG_LINES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogLine {
    pub ts: i64,
    pub level: LogLevel,
    pub code: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Running => "running",
            TaskStatus::Succeeded => "succeeded",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        task_id: String,
        plugin_id: String,
        function: String,
        started_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    Logs {
        task_id: String,
        lines: Vec<LogLine>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        task_id: String,
        done: u64,
        total: u64,
    },
    #[serde(rename_all = "camelCase")]
    Stage { task_id: String, code: String },
    #[serde(rename_all = "camelCase")]
    Finished {
        task_id: String,
        status: TaskStatus,
        result: Option<Value>,
        error: Option<AppError>,
        elapsed_ms: u64,
    },
}

/// 事件出口（Tauri Channel、测试收集器等）
pub trait EventSink: Send + Sync {
    fn emit(&self, event: TaskEvent);
}

struct Throttle {
    pending: Option<(u64, u64)>,
    last: Option<Instant>,
}

/// 任务运行时上下文，实现插件侧的 [`TaskContext`]
pub struct TaskRunner {
    task_id: String,
    plugin_id: String,
    resources: Option<Arc<Resources>>,
    sink: Arc<dyn EventSink>,
    cancel: Arc<AtomicBool>,
    buffer: Mutex<Vec<LogLine>>,
    file: Mutex<Option<BufWriter<File>>>,
    progress: Mutex<Throttle>,
}

impl TaskRunner {
    fn new(
        task_id: String,
        plugin_id: String,
        resources: Option<Arc<Resources>>,
        sink: Arc<dyn EventSink>,
        cancel: Arc<AtomicBool>,
        log_path: &Path,
    ) -> Self {
        let file = File::create(log_path).ok().map(BufWriter::new);
        Self {
            task_id,
            plugin_id,
            resources,
            sink,
            cancel,
            buffer: Mutex::new(Vec::new()),
            file: Mutex::new(file),
            progress: Mutex::new(Throttle {
                pending: None,
                last: None,
            }),
        }
    }

    /// 推送缓冲中的日志与最新进度
    pub fn flush(&self) {
        let lines = std::mem::take(&mut *self.buffer.lock().unwrap_or_else(|e| e.into_inner()));
        if !lines.is_empty() {
            self.sink.emit(TaskEvent::Logs {
                task_id: self.task_id.clone(),
                lines,
            });
        }
        let pending = self
            .progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .take();
        if let Some((done, total)) = pending {
            self.emit_progress(done, total);
        }
        if let Some(file) = self.file.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = file.flush();
        }
    }

    fn emit_progress(&self, done: u64, total: u64) {
        self.sink.emit(TaskEvent::Progress {
            task_id: self.task_id.clone(),
            done,
            total,
        });
    }
}

impl TaskContext for TaskRunner {
    fn log(&self, level: LogLevel, code: &str, params: Value) {
        let line = LogLine {
            ts: now_millis(),
            level,
            code: code.to_owned(),
            params,
        };
        if let Some(file) = self.file.lock().unwrap_or_else(|e| e.into_inner()).as_mut()
            && let Ok(json) = serde_json::to_string(&line)
        {
            let _ = writeln!(file, "{json}");
        }
        let full = {
            let mut buffer = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buffer.push(line);
            buffer.len() >= FLUSH_LINES
        };
        if full {
            self.flush();
        }
    }

    fn progress(&self, done: u64, total: u64) {
        let due = {
            let mut throttle = self.progress.lock().unwrap_or_else(|e| e.into_inner());
            let due = done >= total
                || throttle
                    .last
                    .is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL);
            if due {
                throttle.last = Some(Instant::now());
                throttle.pending = None;
            } else {
                throttle.pending = Some((done, total));
            }
            due
        };
        if due {
            self.emit_progress(done, total);
        }
    }

    fn stage(&self, code: &str) {
        self.flush();
        self.sink.emit(TaskEvent::Stage {
            task_id: self.task_id.clone(),
            code: code.to_owned(),
        });
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|e| io_error(e, path))
    }

    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| io_error(e, path))
    }

    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        self.resources
            .as_ref()
            .and_then(|resources| resources.path(&self.plugin_id, id))
            .ok_or_else(|| PluginError::new("resource.missing").with("id", id))
    }
}

fn io_error(err: std::io::Error, path: &str) -> PluginError {
    let code = match err.kind() {
        std::io::ErrorKind::NotFound => "fs.not_found",
        std::io::ErrorKind::PermissionDenied => "fs.permission_denied",
        _ => "fs.io",
    };
    PluginError::new(code)
        .with("path", path)
        .with("detail", err.to_string())
}

pub struct TaskManager {
    store: Arc<Store>,
    resources: Option<Arc<Resources>>,
    log_dir: PathBuf,
    running: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    seq: AtomicU64,
}

impl TaskManager {
    /// 创建管理器；上次未正常结束的任务标记为中断，并清理过旧的任务与日志
    pub fn new(store: Arc<Store>, log_dir: PathBuf, keep: usize) -> AppResult<Self> {
        std::fs::create_dir_all(&log_dir)?;
        store.tasks_mark_interrupted()?;
        for id in store.tasks_prune(keep)? {
            let _ = std::fs::remove_file(log_dir.join(format!("{id}.jsonl")));
        }
        Ok(Self {
            store,
            resources: None,
            log_dir,
            running: Arc::default(),
            seq: AtomicU64::new(0),
        })
    }

    /// 让任务能够通过 `resource_path` 访问已下载的插件资源
    pub fn with_resources(mut self, resources: Arc<Resources>) -> Self {
        self.resources = Some(resources);
        self
    }

    fn log_path(&self, task_id: &str) -> PathBuf {
        self.log_dir.join(format!("{task_id}.jsonl"))
    }

    /// 启动任务，立即返回任务 id；`job` 在独立线程中执行
    pub fn start<F>(
        &self,
        plugin_id: &str,
        function: &str,
        sink: Arc<dyn EventSink>,
        job: F,
    ) -> AppResult<String>
    where
        F: FnOnce(&TaskRunner) -> AppResult<Value> + Send + 'static,
    {
        let started_at = now_millis();
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let task_id = format!("{started_at:x}-{seq}");
        self.store.task_insert(&TaskRecord {
            id: task_id.clone(),
            plugin_id: plugin_id.to_owned(),
            function: function.to_owned(),
            status: TaskStatus::Running.as_str().to_owned(),
            started_at,
            finished_at: None,
            elapsed_ms: None,
            error_code: None,
        })?;

        let cancel = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(task_id.clone(), cancel.clone());
        sink.emit(TaskEvent::Started {
            task_id: task_id.clone(),
            plugin_id: plugin_id.to_owned(),
            function: function.to_owned(),
            started_at,
        });

        let runner = Arc::new(TaskRunner::new(
            task_id.clone(),
            plugin_id.to_owned(),
            self.resources.clone(),
            sink.clone(),
            cancel.clone(),
            &self.log_path(&task_id),
        ));
        let store = self.store.clone();
        let running = self.running.clone();
        let id = task_id.clone();

        thread::Builder::new()
            .name(format!("task-{task_id}"))
            .spawn(move || {
                let begin = Instant::now();
                let done = Arc::new(AtomicBool::new(false));
                let flusher = {
                    let runner = runner.clone();
                    let done = done.clone();
                    thread::spawn(move || {
                        while !done.load(Ordering::Relaxed) {
                            thread::sleep(FLUSH_INTERVAL);
                            runner.flush();
                        }
                    })
                };

                runner.log(LogLevel::Info, "task.started", Value::Null);
                let outcome = catch_unwind(AssertUnwindSafe(|| job(&runner)))
                    .unwrap_or_else(|_| Err(AppError::new("task.panicked")));
                let (status, result, error) = match outcome {
                    Ok(value) => (TaskStatus::Succeeded, Some(value), None),
                    Err(err) if cancel.load(Ordering::Relaxed) || err.code == "task.cancelled" => {
                        (TaskStatus::Cancelled, None, None)
                    }
                    Err(err) => (TaskStatus::Failed, None, Some(err)),
                };
                let elapsed_ms = begin.elapsed().as_millis() as u64;
                match &error {
                    Some(err) => runner.log(
                        LogLevel::Error,
                        "task.failed",
                        serde_json::json!({ "code": err.code, "params": err.params }),
                    ),
                    None => runner.log(
                        LogLevel::Info,
                        &format!("task.{}", status.as_str()),
                        serde_json::json!({ "elapsedMs": elapsed_ms }),
                    ),
                }

                done.store(true, Ordering::Relaxed);
                let _ = flusher.join();
                runner.flush();

                let _ = store.task_finish(
                    &id,
                    status.as_str(),
                    elapsed_ms as i64,
                    error.as_ref().map(|e| e.code.as_str()),
                );
                running
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                runner.sink.emit(TaskEvent::Finished {
                    task_id: id,
                    status,
                    result,
                    error,
                    elapsed_ms,
                });
            })
            .map_err(|e| AppError::new("task.spawn_failed").with("detail", e.to_string()))?;

        Ok(task_id)
    }

    /// 请求取消；任务在下一次检查 `is_cancelled` 时退出
    pub fn cancel(&self, task_id: &str) -> bool {
        match self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(task_id)
        {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    pub fn list(&self, limit: usize) -> AppResult<Vec<TaskRecord>> {
        self.store.tasks(limit)
    }

    /// 读取任务日志（最多最后 10000 行）
    pub fn logs(&self, task_id: &str) -> AppResult<Vec<LogLine>> {
        if !task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(AppError::new("task.not_found"));
        }
        let file = match File::open(self.log_path(task_id)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut lines: Vec<LogLine> = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect();
        if lines.len() > MAX_LOG_LINES {
            lines.drain(..lines.len() - MAX_LOG_LINES);
        }
        Ok(lines)
    }
}

#[cfg(test)]
#[path = "task_test.rs"]
mod tests;
