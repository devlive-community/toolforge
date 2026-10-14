//! 后台采集：一个聚合线程负责启动平台采集、每秒更新时钟与前台应用、定时落盘。

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tf_plugin_api::PluginResult;

use crate::platform;
use crate::stats::{App, Recorder};
use crate::store::Store;

/// 写入数据库的间隔
const FLUSH_EVERY: Duration = Duration::from_secs(30);
/// 刷新显示器信息的间隔
const DISPLAYS_EVERY: Duration = Duration::from_secs(10);
/// 采集启动失败（如尚未授权）后重试的间隔
const RETRY_EVERY: Duration = Duration::from_secs(3);
/// 停止时最长等待一个周期
const POLL: Duration = Duration::from_millis(250);

/// 采集状态，供界面展示
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// 平台采集正在运行
    pub capturing: bool,
    /// 最近一次失败的错误码
    pub error: Option<String>,
    pub app: Option<App>,
}

/// 聚合线程与插件调用共享的数据
pub struct Shared {
    pub store: Mutex<Store>,
    pub recorder: Mutex<Recorder>,
    pub status: Mutex<Status>,
}

impl Shared {
    pub fn new(store: Store) -> Self {
        Self {
            store: Mutex::new(store),
            recorder: Mutex::new(Recorder::default()),
            status: Mutex::new(Status::default()),
        }
    }

    /// 把内存中的计数写入数据库；查询前调用，保证看到最新数据
    pub fn flush(&self) -> PluginResult<()> {
        let pending = self.recorder.lock().expect("recorder poisoned").take();
        self.store.lock().expect("store poisoned").flush(&pending)
    }

    pub fn set_excluded(&self, excluded: HashSet<String>) {
        self.recorder
            .lock()
            .expect("recorder poisoned")
            .set_excluded(excluded);
    }
}

/// 本地日期与小时
fn clock() -> (String, u8) {
    let now = jiff::Zoned::now();
    (now.date().to_string(), now.hour() as u8)
}

pub struct Collector {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Collector {
    pub fn start(shared: Arc<Shared>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let worker = std::thread::Builder::new()
            .name("input-stats".into())
            .spawn(move || run(&shared, &flag))
            .ok();
        Self { stop, worker }
    }

    /// 停止采集并等待最后一次落盘
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(shared: &Shared, stop: &AtomicBool) {
    let started = Instant::now();
    let mut capture: Option<(mpsc::Receiver<crate::stats::Event>, platform::Stopper)> = None;
    let mut last_attempt: Option<Instant> = None;
    let mut last_tick = Instant::now();
    let mut last_flush = Instant::now();
    let mut last_displays: Option<Instant> = None;

    let tick = |recorder: &mut Recorder, elapsed: u64| {
        let (day, hour) = clock();
        recorder.set_clock(&day, hour);
        let app = platform::foreground_app();
        recorder.set_app(app.clone());
        recorder.tick(started.elapsed().as_secs(), elapsed);
        shared.status.lock().expect("status poisoned").app = app;
    };
    tick(&mut shared.recorder.lock().expect("recorder poisoned"), 0);

    while !stop.load(Ordering::SeqCst) {
        // 采集未运行时定期尝试启动（例如用户刚刚授权）
        if capture.is_none() && last_attempt.is_none_or(|t| t.elapsed() >= RETRY_EVERY) {
            last_attempt = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            let result = platform::start(tx);
            let mut status = shared.status.lock().expect("status poisoned");
            match result {
                Ok(stopper) => {
                    capture = Some((rx, stopper));
                    status.capturing = true;
                    status.error = None;
                }
                Err(err) => {
                    status.capturing = false;
                    status.error = Some(err.code);
                }
            }
        }

        let mut lost = false;
        if let Some((rx, _)) = &capture {
            match rx.recv_timeout(POLL) {
                Ok(first) => {
                    let now = started.elapsed().as_secs();
                    let mut recorder = shared.recorder.lock().expect("recorder poisoned");
                    recorder.record(first, now);
                    for event in rx.try_iter() {
                        recorder.record(event, now);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => lost = true,
            }
        } else {
            std::thread::sleep(POLL);
        }
        if lost {
            // 采集线程意外退出：稍后重新启动
            if let Some((_, stopper)) = capture.take() {
                stopper();
            }
            shared.status.lock().expect("status poisoned").capturing = false;
        }

        if last_displays.is_none_or(|t| t.elapsed() >= DISPLAYS_EVERY) {
            last_displays = Some(Instant::now());
            let displays = platform::displays();
            shared
                .recorder
                .lock()
                .expect("recorder poisoned")
                .set_displays(displays);
        }
        let elapsed = last_tick.elapsed().as_secs();
        if elapsed >= 1 {
            last_tick += Duration::from_secs(elapsed);
            tick(
                &mut shared.recorder.lock().expect("recorder poisoned"),
                elapsed,
            );
        }
        if last_flush.elapsed() >= FLUSH_EVERY {
            last_flush = Instant::now();
            record_flush(shared);
        }
    }

    if let Some((rx, stopper)) = capture.take() {
        stopper();
        let now = started.elapsed().as_secs();
        let mut recorder = shared.recorder.lock().expect("recorder poisoned");
        for event in rx.try_iter() {
            recorder.record(event, now);
        }
    }
    record_flush(shared);
    let mut status = shared.status.lock().expect("status poisoned");
    status.capturing = false;
    status.app = None;
}

fn record_flush(shared: &Shared) {
    if let Err(err) = shared.flush() {
        shared.status.lock().expect("status poisoned").error = Some(err.code);
    }
}
