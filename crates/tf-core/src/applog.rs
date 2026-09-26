//! 应用日志：写入 `<数据目录>/logs/app.log`，超过大小上限时轮转（app.1.log、app.2.log…），
//! 通过 `log` 门面接收宿主与插件的日志，并过滤依赖库的噪音。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::store::now_millis;

const FILE: &str = "app";
/// 本项目自己的日志目标前缀；其他库只记录警告及以上
const OWN_TARGETS: &[&str] = &["toolforge", "tf_", "tfp_", "webview"];

pub struct AppLog {
    dir: PathBuf,
    max_bytes: u64,
    keep: usize,
    file: Mutex<Option<File>>,
}

/// Unix 毫秒 → `YYYY-MM-DDTHH:MM:SS.mmmZ`（UTC）
pub fn format_time(millis: i64) -> String {
    let secs = millis.div_euclid(1000);
    let ms = millis.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant 的 civil_from_days 算法
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

impl AppLog {
    pub fn new(dir: impl Into<PathBuf>, max_bytes: u64, keep: usize) -> Self {
        Self {
            dir: dir.into(),
            max_bytes,
            keep: keep.max(1),
            file: Mutex::new(None),
        }
    }

    fn path(&self, index: usize) -> PathBuf {
        if index == 0 {
            self.dir.join(format!("{FILE}.log"))
        } else {
            self.dir.join(format!("{FILE}.{index}.log"))
        }
    }

    /// 现有日志文件，最新的在前
    pub fn files(&self) -> Vec<PathBuf> {
        (0..self.keep)
            .map(|i| self.path(i))
            .filter(|p| p.exists())
            .collect()
    }

    fn rotate(&self) {
        let _ = std::fs::remove_file(self.path(self.keep - 1));
        for index in (0..self.keep - 1).rev() {
            let _ = std::fs::rename(self.path(index), self.path(index + 1));
        }
    }

    fn open(&self) -> Option<File> {
        std::fs::create_dir_all(&self.dir).ok()?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(0))
            .ok()
    }

    /// 写入一行日志；消息中的换行会被折叠，保证一条日志占一行
    pub fn write(&self, level: &str, target: &str, message: &str) {
        let line = format!(
            "{} {level:<5} {target} {}\n",
            format_time(now_millis()),
            message.replace("\r\n", " ⏎ ").replace('\n', " ⏎ ")
        );
        #[cfg(debug_assertions)]
        eprint!("{line}");
        let mut guard = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let too_big = guard
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .is_some_and(|m| m.len() + line.len() as u64 > self.max_bytes);
        if too_big {
            *guard = None;
            self.rotate();
        }
        if guard.is_none() {
            *guard = self.open();
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    /// 注册为全局 `log` 实现；已注册过其他实现时只返回自身，仍可直接写入
    pub fn install(self) -> &'static AppLog {
        let logger: &'static AppLog = Box::leak(Box::new(self));
        if log::set_logger(logger).is_ok() {
            log::set_max_level(LevelFilter::Info);
        }
        logger
    }

    /// 立即把缓冲写入磁盘（导出诊断包前调用）
    pub fn flush_now(&self) {
        Log::flush(self);
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

fn own(target: &str) -> bool {
    OWN_TARGETS.iter().any(|prefix| target.starts_with(prefix))
}

impl Log for AppLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let limit = if own(metadata.target()) {
            Level::Info
        } else {
            Level::Warn
        };
        metadata.level() <= limit
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            self.write(
                record.level().as_str(),
                record.target(),
                &record.args().to_string(),
            );
        }
    }

    fn flush(&self) {
        if let Some(file) = self.file.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = file.flush();
        }
    }
}

#[cfg(test)]
#[path = "applog_test.rs"]
mod tests;
