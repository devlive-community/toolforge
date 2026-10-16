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
fn plugin_state_is_scoped_and_validated() {
    let store = Store::open_in_memory().unwrap();
    store.plugin_state_set("a", "draft", &json!("hi")).unwrap();
    assert_eq!(
        store.plugin_state_get("a", "draft").unwrap(),
        Some(json!("hi"))
    );
    assert_eq!(store.plugin_state_get("b", "draft").unwrap(), None);
    // null 删除
    store.plugin_state_set("a", "draft", &Value::Null).unwrap();
    assert_eq!(store.plugin_state_get("a", "draft").unwrap(), None);

    for bad in ["", "a:b", "../x", &"k".repeat(65)] {
        assert_eq!(
            store.plugin_state_get("a", bad).unwrap_err().code,
            "state.invalid_key"
        );
    }
    let big = json!("x".repeat(PLUGIN_STATE_LIMIT));
    assert_eq!(
        store.plugin_state_set("a", "big", &big).unwrap_err().code,
        "state.too_large"
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
        rerunnable: false,
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

#[test]
fn history_merges_bursts_and_searches() {
    let store = Store::open_in_memory().unwrap();
    let t0 = 1_000_000;
    let a = store
        .history_record("json", "format", "{", None, t0)
        .unwrap();
    let b = store
        .history_record("json", "format", "{\"a\":1}", Some("ok"), t0 + 5_000)
        .unwrap();
    assert_eq!(a, b, "same burst merges");
    let c = store
        .history_record("json", "minify", "{}", None, t0 + 6_000)
        .unwrap();
    let d = store
        .history_record("json", "format", "[1]", None, t0 + 7_000)
        .unwrap();
    assert!(c != a && d != c, "another function breaks the burst");
    let e = store
        .history_record(
            "json",
            "format",
            "[2]",
            None,
            t0 + 7_000 + HISTORY_MERGE_MS + 1,
        )
        .unwrap();
    assert_ne!(e, d, "a pause starts a new entry");

    let all = store.history_list("", 10, 0).unwrap();
    assert_eq!(all.len(), 4);
    assert_eq!(all[0].input, "[2]");
    assert_eq!(all[3].input, "{\"a\":1}");
    assert_eq!(all[3].output.as_deref(), Some("ok"));
    assert_eq!(all[3].created_at, t0);
    assert_eq!(store.history_list("\"a\"", 10, 0).unwrap().len(), 1);
    assert_eq!(
        store.history_list("%", 10, 0).unwrap().len(),
        0,
        "LIKE wildcards are escaped"
    );

    store.history_delete(e).unwrap();
    assert_eq!(store.history_count().unwrap(), 3);
    assert_eq!(store.history_prune(2).unwrap(), 1);
    assert_eq!(store.history_clear(Some("json")).unwrap(), 2);
}

#[test]
fn task_args_enable_rerun() {
    let store = Store::open_in_memory().unwrap();
    let record = TaskRecord {
        id: "t1".into(),
        plugin_id: "hash".into(),
        function: "hash_files".into(),
        status: "running".into(),
        started_at: 1,
        finished_at: None,
        elapsed_ms: None,
        error_code: None,
        rerunnable: false,
    };
    store.task_insert(&record).unwrap();
    assert!(!store.tasks(10).unwrap()[0].rerunnable);
    store.task_set_args("t1", &json!({"paths": ["a"]})).unwrap();
    assert!(store.tasks(10).unwrap()[0].rerunnable);
    let (plugin, function, args) = store.task_args("t1").unwrap().unwrap();
    assert_eq!(
        (plugin.as_str(), function.as_str(), args),
        ("hash", "hash_files", json!({"paths": ["a"]}))
    );
    assert!(
        store.tasks_clear_finished().unwrap().is_empty(),
        "running tasks stay"
    );
    store.task_finish("t1", "succeeded", 5, None).unwrap();
    assert_eq!(store.tasks_clear_finished().unwrap(), vec!["t1"]);
}
