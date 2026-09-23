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
