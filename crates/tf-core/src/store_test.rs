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
        error_params: Default::default(),
        rerunnable: false,
    }
}

#[test]
fn task_lifecycle_is_persisted() {
    let store = Store::open_in_memory().unwrap();
    store.task_insert(&record("t1", 1)).unwrap();
    store
        .task_finish("t1", "failed", 42, Some(&AppError::new("fs.not_found")))
        .unwrap();
    let tasks = store.tasks(10, 0).unwrap();
    assert_eq!(tasks[0].status, "failed");
    assert_eq!(tasks[0].elapsed_ms, Some(42));
    assert_eq!(tasks[0].error_code.as_deref(), Some("fs.not_found"));
}

#[test]
fn task_error_parameters_survive_reopening_the_database() {
    let dir = std::env::temp_dir().join(format!(
        "tf-store-errors-{}-{:?}",
        now_millis(),
        std::thread::current().id()
    ));
    let path = dir.join("state.db");
    let error = AppError::new("resource.network")
        .with("host", "models.example.test")
        .with("detail", "connection reset");
    {
        let store = Store::open(&path).unwrap();
        store.task_insert(&record("download", 1)).unwrap();
        store
            .task_finish("download", "failed", 223, Some(&error))
            .unwrap();
    }
    {
        let store = Store::open(&path).unwrap();
        let task = store.tasks(1, 0).unwrap().remove(0);
        assert_eq!(task.error_code.as_deref(), Some("resource.network"));
        assert_eq!(task.error_params, error.params);
        let json = serde_json::to_value(task).unwrap();
        assert_eq!(json["errorParams"]["host"], "models.example.test");
        store
            .task_finish("download", "succeeded", 400, None)
            .unwrap();
        let task = store.tasks(1, 0).unwrap().remove(0);
        assert!(task.error_code.is_none() && task.error_params.is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn task_error_migration_preserves_existing_records_and_preferences() {
    let connection = Connection::open_in_memory().unwrap();
    for sql in &MIGRATIONS[..3] {
        connection.execute_batch(sql).unwrap();
    }
    connection.pragma_update(None, "user_version", 3).unwrap();
    connection
        .execute(
            "INSERT INTO tasks (id, plugin_id, function, status, started_at, error_code)
         VALUES ('old', 'p', 'download_resource', 'failed', 1, 'resource.network')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO kv (key, value, updated_at) VALUES ('pref', '42', 1)",
            [],
        )
        .unwrap();
    let store = Store::init(connection).unwrap();
    let task = store.tasks(10, 0).unwrap().remove(0);
    assert_eq!(task.id, "old");
    assert_eq!(task.error_code.as_deref(), Some("resource.network"));
    assert!(task.error_params.is_empty());
    assert_eq!(store.kv_get("pref").unwrap(), Some(json!(42)));
    assert_eq!(store.tasks_count().unwrap(), 1);
}

#[test]
fn running_tasks_become_interrupted() {
    let store = Store::open_in_memory().unwrap();
    store.task_insert(&record("t1", 1)).unwrap();
    assert_eq!(store.tasks_mark_interrupted().unwrap(), 1);
    assert_eq!(store.tasks(10, 0).unwrap()[0].status, "interrupted");
}

#[test]
fn tasks_page_through_records() {
    let store = Store::open_in_memory().unwrap();
    for i in 0..5 {
        store.task_insert(&record(&format!("t{i}"), i)).unwrap();
    }
    assert_eq!(store.tasks_count().unwrap(), 5);
    let page1 = store.tasks(2, 0).unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].id, "t4", "newest first");
    let page2 = store.tasks(2, 2).unwrap();
    assert_eq!(page2.len(), 2);
    assert_eq!(page2[0].id, "t2");
    let last = store.tasks(2, 4).unwrap();
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].id, "t0");
    // 记录永久保留：不分页裁剪
    assert_eq!(store.tasks(100, 0).unwrap().len(), 5);
}

#[test]
fn history_records_every_call_and_searches() {
    let store = Store::open_in_memory().unwrap();
    let t0 = 1_000_000;
    let calls = [
        ("json", "format", "{", None),
        ("json", "format", "{\"a\":1}", Some("ok")),
        ("json", "minify", "{}", None),
        ("json", "format", "[1]", None),
        ("json", "format", "[2]", None),
    ];
    let ids: Vec<i64> = calls
        .iter()
        .enumerate()
        .map(|(i, (plugin, function, input, output))| {
            store
                .history_record(plugin, function, input, *output, t0 + i as i64 * 1_000)
                .unwrap()
        })
        .collect();
    // 每次调用都单独成条，id 严格递增（自增主键）
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "every call is its own entry"
    );

    let all = store.history_list("", 10, 0).unwrap();
    assert_eq!(all.len(), 5);
    assert_eq!(all[0].input, "[2]");
    assert_eq!(all[4].input, "{");
    assert_eq!(all[4].created_at, t0);
    assert_eq!(all[3].output.as_deref(), Some("ok"));
    assert_eq!(store.history_list("\"a\"", 10, 0).unwrap().len(), 1);
    assert_eq!(
        store.history_list("%", 10, 0).unwrap().len(),
        0,
        "LIKE wildcards are escaped"
    );

    store.history_delete(ids[4]).unwrap();
    assert_eq!(store.history_count().unwrap(), 4);
    assert_eq!(store.history_clear(Some("json")).unwrap(), 4);
    assert_eq!(store.history_count().unwrap(), 0);
    // 历史记录永久保留：写入的记录不会因数量被裁剪（上方 all.len() == 5 即验证）
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
        error_params: Default::default(),
        rerunnable: false,
    };
    store.task_insert(&record).unwrap();
    assert!(!store.tasks(10, 0).unwrap()[0].rerunnable);
    store.task_set_args("t1", &json!({"paths": ["a"]})).unwrap();
    assert!(store.tasks(10, 0).unwrap()[0].rerunnable);
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

fn download_report(id: &str, now: i64) -> DownloadReport {
    DownloadReport {
        attempt_id: id.into(),
        receipt: "test-only-receipt".into(),
        sha256: "a".repeat(64),
        size_bytes: 42,
        expires_at: now + 7 * 24 * 60 * 60 * 1000,
        created_at: now,
        next_attempt_at: now,
        attempts: 0,
    }
}

#[test]
fn download_outbox_is_idempotent_and_preserves_retry_state() {
    let store = Store::open_in_memory().unwrap();
    let now = 1_000_000_000;
    let report = download_report("attempt", now);
    store.download_report_enqueue(&report).unwrap();
    store
        .download_report_retry("attempt", now + 60_000)
        .unwrap();
    store.download_report_enqueue(&report).unwrap();
    assert!(store.download_reports_due(now, 10).unwrap().is_empty());
    let reports = store.download_reports_due(now + 60_000, 10).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].attempts, 1);
    assert_eq!(reports[0].receipt, "test-only-receipt");
    store.download_report_remove("attempt").unwrap();
    assert!(
        store
            .download_reports_due(now + 60_000, 10)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn download_outbox_survives_reopen_and_expires_before_receipt() {
    let dir = std::env::temp_dir().join(format!(
        "tf-outbox-{}-{:?}",
        now_millis(),
        std::thread::current().id()
    ));
    let path = dir.join("state.db");
    let now = 1_000_000_000;
    {
        let store = Store::open(&path).unwrap();
        store
            .download_report_enqueue(&download_report("durable", now))
            .unwrap();
        let mut expired = download_report("expired", now);
        expired.expires_at = now + 1000;
        store.download_report_enqueue(&expired).unwrap();
    }
    {
        let store = Store::open(&path).unwrap();
        let reports = store.download_reports_due(now + 1001, 10).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].attempt_id, "durable");
        assert!(
            store
                .download_reports_due(now + 5 * 24 * 60 * 60 * 1000, 10)
                .unwrap()
                .is_empty()
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn download_outbox_migration_preserves_version_four_state() {
    let conn = Connection::open_in_memory().unwrap();
    for sql in &MIGRATIONS[..4] {
        conn.execute_batch(sql).unwrap();
    }
    conn.pragma_update(None, "user_version", 4).unwrap();
    conn.execute("INSERT INTO kv VALUES ('prefs', '42', 1)", [])
        .unwrap();
    let store = Store::init(conn).unwrap();
    assert_eq!(store.kv_get("prefs").unwrap(), Some(json!(42)));
    store
        .download_report_enqueue(&download_report("migrated", 1_000_000_000))
        .unwrap();
    assert_eq!(
        store.download_reports_due(1_000_000_000, 1).unwrap().len(),
        1
    );
}

#[test]
fn interface_grants_are_kept_per_channel() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.interface_grants("agent").unwrap().is_empty());
    let mut grant = InterfaceGrant {
        channel: "agent".into(),
        plugin_id: "org.devlive.toolforge.hash".into(),
        function: PLUGIN_SCOPE.into(),
        mode: "off".into(),
        uses: None,
    };
    store.interface_grant_set(&grant).unwrap();
    grant.function = "hash_files".into();
    grant.mode = "ask".into();
    grant.uses = Some(vec!["fs:user-selected".into()]);
    store.interface_grant_set(&grant).unwrap();
    grant.mode = "allow".into();
    store.interface_grant_set(&grant).unwrap();
    let cli = InterfaceGrant {
        channel: "cli".into(),
        mode: "off".into(),
        ..grant.clone()
    };
    store.interface_grant_set(&cli).unwrap();

    let grants = store.interface_grants("agent").unwrap();
    assert_eq!(grants.len(), 2);
    assert_eq!(grants[0].function, PLUGIN_SCOPE);
    assert_eq!(grants[1].mode, "allow");
    assert_eq!(
        grants[1].uses.as_deref(),
        Some(&["fs:user-selected".to_owned()][..])
    );
    assert_eq!(
        store.interface_grants("cli").unwrap(),
        std::slice::from_ref(&cli)
    );

    let bad = InterfaceGrant {
        mode: "always".into(),
        ..grant.clone()
    };
    assert!(
        store.interface_grant_set(&bad).is_err(),
        "unknown modes are rejected by the schema"
    );
    let bad = InterfaceGrant {
        channel: "web".into(),
        ..grant.clone()
    };
    assert!(
        store.interface_grant_set(&bad).is_err(),
        "unknown channels are rejected by the schema"
    );

    store
        .interface_grant_remove("agent", &grant.plugin_id, "hash_files")
        .unwrap();
    assert_eq!(store.interface_grants("agent").unwrap().len(), 1);
    assert_eq!(store.interface_grants("cli").unwrap().len(), 1);
}

/// 第 6 步建的 agent_grants 在第 7 步迁移为 agent 通道的设置
#[test]
fn migrates_agent_grants_into_interface_grants() {
    let path = std::env::temp_dir().join(format!("tf-store-grants-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        for sql in &MIGRATIONS[..6] {
            tx.execute_batch(sql).unwrap();
        }
        tx.pragma_update(None, "user_version", 6).unwrap();
        tx.execute(
            "INSERT INTO agent_grants VALUES ('p', '*', 'allow', NULL, 1)",
            [],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let store = Store::open(&path).unwrap();
    let grants = store.interface_grants("agent").unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(
        (grants[0].plugin_id.as_str(), grants[0].mode.as_str()),
        ("p", "allow")
    );
    drop(store);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn agent_paths_are_upserted_and_removed() {
    let store = Store::open_in_memory().unwrap();
    let path = AgentPath {
        path: "/work".into(),
        access: "read".into(),
    };
    store.agent_path_set(&path).unwrap();
    store
        .agent_path_set(&AgentPath {
            access: "write".into(),
            ..path.clone()
        })
        .unwrap();
    assert_eq!(
        store.agent_paths().unwrap(),
        [AgentPath {
            path: "/work".into(),
            access: "write".into()
        }]
    );
    store.agent_path_remove("/work").unwrap();
    assert!(store.agent_paths().unwrap().is_empty());
}

#[test]
fn history_keeps_the_call_source() {
    let store = Store::open_in_memory().unwrap();
    store.history_record("p", "f", "from app", None, 1).unwrap();
    store
        .history_record_from("agent", "p", "f", "from agent", None, 2)
        .unwrap();
    let entries = store.history_list("", 10, 0).unwrap();
    assert_eq!(entries[0].source, "agent");
    assert_eq!(entries[1].source, "app");
}
