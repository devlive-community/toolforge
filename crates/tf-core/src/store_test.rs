use super::*;
use serde_json::json;

#[test]
fn kv_roundtrip_and_overwrite() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.kv_get("prefs").unwrap(), None);
    store.kv_set("prefs", &json!({"theme": "dark"})).unwrap();
    store.kv_set("prefs", &json!({"theme": "light"})).unwrap();
    assert_eq!(
        store.kv_get("prefs").unwrap(),
        Some(json!({"theme": "light"}))
    );
}

#[test]
fn favorite_toggles() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.toggle_favorite("a").unwrap());
    assert_eq!(store.favorites().unwrap(), vec!["a"]);
    assert!(!store.toggle_favorite("a").unwrap());
    assert!(store.favorites().unwrap().is_empty());
}

#[test]
fn recent_orders_by_latest_open() {
    let store = Store::open_in_memory().unwrap();
    store.touch_recent("a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    store.touch_recent("b").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    store.touch_recent("a").unwrap();
    assert_eq!(store.recent(10).unwrap(), vec!["a", "b"]);
    assert_eq!(store.recent(1).unwrap(), vec!["a"]);
}

#[test]
fn migrations_are_idempotent() {
    let dir = std::env::temp_dir().join(format!("tf-store-{}", now_millis()));
    let path = dir.join("state.db");
    Store::open(&path).unwrap().kv_set("k", &json!(1)).unwrap();
    assert_eq!(
        Store::open(&path).unwrap().kv_get("k").unwrap(),
        Some(json!(1))
    );
    std::fs::remove_dir_all(dir).ok();
}

fn record(id: &str, started_at: i64) -> TaskRecord {
    TaskRecord {
        id: id.into(),
        plugin_id: "p".into(),
        function: "f".into(),
        status: "running".into(),
        started_at,
        finished_at: None,
        elapsed_ms: None,
        error_code: None,
    }
}

#[test]
fn task_lifecycle_is_persisted() {
    let store = Store::open_in_memory().unwrap();
    store.task_insert(&record("t1", 1)).unwrap();
    store
        .task_finish("t1", "failed", 42, Some("fs.not_found"))
        .unwrap();
    let tasks = store.tasks(10).unwrap();
    assert_eq!(tasks[0].status, "failed");
    assert_eq!(tasks[0].elapsed_ms, Some(42));
    assert_eq!(tasks[0].error_code.as_deref(), Some("fs.not_found"));
}

#[test]
fn running_tasks_become_interrupted() {
    let store = Store::open_in_memory().unwrap();
    store.task_insert(&record("t1", 1)).unwrap();
    assert_eq!(store.tasks_mark_interrupted().unwrap(), 1);
    assert_eq!(store.tasks(10).unwrap()[0].status, "interrupted");
}

#[test]
fn prune_keeps_latest_tasks() {
    let store = Store::open_in_memory().unwrap();
    for i in 0..5 {
        store.task_insert(&record(&format!("t{i}"), i)).unwrap();
    }
    let removed = store.tasks_prune(2).unwrap();
    assert_eq!(removed.len(), 3);
    let left: Vec<String> = store.tasks(10).unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(left, vec!["t4", "t3"]);
}
