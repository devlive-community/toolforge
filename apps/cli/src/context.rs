//! 命令行里的任务运行环境：阶段与进度在终端（stderr）同一行刷新，
//! 警告和错误日志总是输出，其余日志只在 `--verbose` 时输出；Ctrl+C 请求取消。

use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tf_core::Resources;
use tf_plugin_api::{LogLevel, PluginError, PluginResult, TaskContext};

use crate::i18n::Messages;

const REFRESH: Duration = Duration::from_millis(100);

pub struct CliContext<'a> {
    messages: &'a Messages,
    plugin_id: String,
    verbose: bool,
    tty: bool,
    cancelled: &'a AtomicBool,
    resources: Option<Resources>,
    line: Mutex<Line>,
}

#[derive(Default)]
struct Line {
    stage: String,
    shown: bool,
    last: Option<Instant>,
}

pub fn io_error(err: &std::io::Error, path: &str) -> PluginError {
    let code = match err.kind() {
        std::io::ErrorKind::NotFound => "fs.not_found",
        std::io::ErrorKind::PermissionDenied => "fs.permission_denied",
        _ => "fs.io",
    };
    PluginError::new(code)
        .with("path", path)
        .with("detail", err.to_string())
}

impl<'a> CliContext<'a> {
    pub fn new(
        messages: &'a Messages,
        plugin_id: &str,
        verbose: bool,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            messages,
            plugin_id: plugin_id.to_owned(),
            verbose,
            tty: std::io::stderr().is_terminal(),
            cancelled,
            resources: tf_host::app_data_dir().map(|dir| Resources::new(dir.join("resources"))),
            line: Mutex::new(Line::default()),
        }
    }

    /// 清掉进度行，之后的输出从行首开始
    pub fn clear(&self) {
        let mut line = self.line.lock().unwrap_or_else(|e| e.into_inner());
        if line.shown {
            eprint!("\r\x1b[2K");
            line.shown = false;
        }
    }

    fn draw(&self, line: &mut Line, text: &str) {
        if !self.tty {
            return;
        }
        eprint!("\r\x1b[2K{text}");
        let _ = std::io::stderr().flush();
        line.shown = true;
        line.last = Some(Instant::now());
    }
}

impl TaskContext for CliContext<'_> {
    fn log(&self, level: LogLevel, code: &str, params: Value) {
        let important = matches!(level, LogLevel::Warn | LogLevel::Error);
        if !important && !self.verbose {
            return;
        }
        let params = match params {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        let text = self
            .messages
            .get(&format!("logs.{code}"), &params)
            .unwrap_or_else(|| code.to_owned());
        self.clear();
        match level {
            LogLevel::Warn => eprintln!("warn: {text}"),
            LogLevel::Error => eprintln!("error: {text}"),
            _ => eprintln!("{text}"),
        }
    }

    fn progress(&self, done: u64, total: u64) {
        let mut line = self.line.lock().unwrap_or_else(|e| e.into_inner());
        let due = line.last.is_none_or(|t| t.elapsed() >= REFRESH) || done >= total;
        if !due {
            return;
        }
        let percent = done.saturating_mul(100).checked_div(total).unwrap_or(0);
        let text = format!("{} {percent}%", line.stage).trim_start().to_owned();
        self.draw(&mut line, &text);
    }

    fn stage(&self, code: &str) {
        let mut line = self.line.lock().unwrap_or_else(|e| e.into_inner());
        line.stage = self
            .messages
            .get(&format!("stages.{code}"), &Map::new())
            .unwrap_or_else(|| code.to_owned());
        let text = line.stage.clone();
        self.draw(&mut line, &text);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn open_file(&self, path: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(path)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|e| io_error(&e, path))
    }

    fn file_size(&self, path: &str) -> PluginResult<u64> {
        std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| io_error(&e, path))
    }

    fn resource_path(&self, id: &str) -> PluginResult<PathBuf> {
        self.resources
            .as_ref()
            .and_then(|resources| resources.path(&self.plugin_id, id))
            .ok_or_else(|| PluginError::new("resource.missing").with("id", id))
    }
}
