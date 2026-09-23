use super::*;
use serde_json::json;

#[derive(Default)]
struct Collector(Mutex<Vec<TaskEvent>>);

impl EventSink for Collector {
    fn emit(&self, event: TaskEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Collector {
    fn wait_finished(&self) -> (TaskStatus, Option<Value>, Option<AppError>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(TaskEvent::Finished {
                status,
                result,
                error,
                ..
            }) = self.0.lock().unwrap().last().cloned()
            {
                return (status, result, error);
            }
            assert!(Instant::now() < deadline, "task did not finish");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn log_codes(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                TaskEvent::Logs { lines, .. } => {
                    Some(lines.iter().map(|l| l.code.clone()).collect::<Vec<_>>())
                }
                _ => None,
            })
            .flatten()
            .collect()
    }
}

fn manager() -> (TaskManager, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "tf-task-{}-{:?}",
        now_millis(),
        thread::current().id()
    ));
    let store = Arc::new(Store::open_in_memory().unwrap());
    (TaskManager::new(store, dir.clone(), 100).unwrap(), dir)
}

#[test]
fn successful_task_streams_logs_progress_and_result() {
    let (manager, dir) = manager();
    let sink = Arc::new(Collector::default());
    let id = manager
        .start("p", "f", sink.clone(), |ctx| {
            ctx.log(LogLevel::Info, "work.step", json!({ "n": 1 }));
            ctx.progress(1, 2);
            ctx.progress(2, 2);
            Ok(json!({ "ok": true }))
        })
        .unwrap();

    let (status, result, error) = sink.wait_finished();
    assert_eq!(status, TaskStatus::Succeeded);
    assert_eq!(result, Some(json!({ "ok": true })));
    assert!(error.is_none());
    assert_eq!(
        sink.log_codes(),
        vec!["task.started", "work.step", "task.succeeded"]
    );
    assert!(sink.0.lock().unwrap().iter().any(|e| matches!(
        e,
        TaskEvent::Progress {
            done: 2,
            total: 2,
            ..
        }
    )));

    // 日志落盘，记录状态已更新
    assert_eq!(manager.logs(&id).unwrap().len(), 3);
    assert_eq!(manager.list(10).unwrap()[0].status, "succeeded");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn cancelled_task_reports_cancelled() {
    let (manager, dir) = manager();
    let sink = Arc::new(Collector::default());
    let id = manager
        .start("p", "f", sink.clone(), |ctx| {
            while !ctx.is_cancelled() {
                thread::sleep(Duration::from_millis(5));
            }
            Err(tf_plugin_api::cancelled().into())
        })
        .unwrap();
    thread::sleep(Duration::from_millis(30));
    assert!(manager.cancel(&id));
    assert_eq!(sink.wait_finished().0, TaskStatus::Cancelled);
    assert!(
        !manager.cancel(&id),
        "finished tasks are no longer cancellable"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn failures_and_panics_are_reported_as_failed() {
    let (manager, dir) = manager();
    let sink = Arc::new(Collector::default());
    manager
        .start("p", "f", sink.clone(), |_| {
            Err(AppError::new("fs.not_found"))
        })
        .unwrap();
    let (status, _, error) = sink.wait_finished();
    assert_eq!(status, TaskStatus::Failed);
    assert_eq!(error.unwrap().code, "fs.not_found");

    let sink = Arc::new(Collector::default());
    manager
        .start("p", "f", sink.clone(), |_| -> AppResult<Value> {
            panic!("boom")
        })
        .unwrap();
    let (status, _, error) = sink.wait_finished();
    assert_eq!(status, TaskStatus::Failed);
    assert_eq!(error.unwrap().code, "task.panicked");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn logs_are_batched() {
    let (manager, dir) = manager();
    let sink = Arc::new(Collector::default());
    manager
        .start("p", "f", sink.clone(), |ctx| {
            for i in 0..500 {
                ctx.log(LogLevel::Debug, "work.line", json!({ "i": i }));
            }
            Ok(Value::Null)
        })
        .unwrap();
    sink.wait_finished();
    let batches = sink
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, TaskEvent::Logs { .. }))
        .count();
    assert_eq!(sink.log_codes().len(), 502);
    assert!(batches <= 5, "expected batched log events, got {batches}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn rejects_suspicious_task_ids_when_reading_logs() {
    let (manager, dir) = manager();
    assert_eq!(
        manager.logs("../../etc/passwd").unwrap_err().code,
        "task.not_found"
    );
    std::fs::remove_dir_all(dir).ok();
}
