//! 本地持久化：只使用 SQLite（前端禁止使用任何浏览器存储）。

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::AppResult;

/// 迁移脚本，按顺序执行；`user_version` 记录已执行到的版本。
const MIGRATIONS: &[&str] = &[r#"
    CREATE TABLE kv (
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE favorites (
        plugin_id  TEXT PRIMARY KEY,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE recent (
        plugin_id TEXT PRIMARY KEY,
        opened_at INTEGER NOT NULL
    );
"#];

pub struct Store {
    conn: Mutex<Connection>,
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

impl Store {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> AppResult<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (index + 1) as i64)?;
            tx.commit()?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // 持锁期间不会 panic，出现中毒时直接复用内部连接
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn kv_get(&self, key: &str) -> AppResult<Option<Value>> {
        let raw: Option<String> = self
            .conn()
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
    }

    pub fn kv_set(&self, key: &str, value: &Value) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value.to_string(), now_millis()],
        )?;
        Ok(())
    }

    pub fn favorites(&self) -> AppResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT plugin_id FROM favorites ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// 切换收藏状态，返回切换后是否已收藏。
    pub fn toggle_favorite(&self, plugin_id: &str) -> AppResult<bool> {
        let conn = self.conn();
        let removed = conn.execute("DELETE FROM favorites WHERE plugin_id = ?1", [plugin_id])?;
        if removed > 0 {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO favorites (plugin_id, created_at) VALUES (?1, ?2)",
            params![plugin_id, now_millis()],
        )?;
        Ok(true)
    }

    pub fn recent(&self, limit: usize) -> AppResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT plugin_id FROM recent ORDER BY opened_at DESC LIMIT ?1")?;
        let rows = stmt.query_map([limit as i64], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn touch_recent(&self, plugin_id: &str) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO recent (plugin_id, opened_at) VALUES (?1, ?2)
             ON CONFLICT(plugin_id) DO UPDATE SET opened_at = excluded.opened_at",
            params![plugin_id, now_millis()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
}
