//! 本地持久化：只使用 SQLite（前端禁止使用任何浏览器存储）。

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

use crate::{AppError, AppResult};

/// 单条插件状态序列化后的大小上限
pub const PLUGIN_STATE_LIMIT: usize = 2 * 1024 * 1024;

/// 迁移脚本，按顺序执行；`user_version` 记录已执行到的版本。
const MIGRATIONS: &[&str] = &[
    r#"
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
"#,
    r#"
    CREATE TABLE tasks (
        id          TEXT PRIMARY KEY,
        plugin_id   TEXT NOT NULL,
        function    TEXT NOT NULL,
        status      TEXT NOT NULL,
        started_at  INTEGER NOT NULL,
        finished_at INTEGER,
        elapsed_ms  INTEGER,
        error_code  TEXT
    );
    CREATE INDEX tasks_started_at ON tasks (started_at DESC);
"#,
];

pub struct Store {
    conn: Mutex<Connection>,
}

/// 任务记录（任务中心与历史）
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub plugin_id: String,
    pub function: String,
    pub status: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub error_code: Option<String>,
}

pub fn now_millis() -> i64 {
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

    pub fn kv_delete(&self, key: &str) -> AppResult<()> {
        self.conn()
            .execute("DELETE FROM kv WHERE key = ?1", [key])?;
        Ok(())
    }

    /// 插件私有状态的存储键：`plugin:<插件 id>:<键>`，键只允许字母、数字与 `._-`
    fn plugin_state_key(plugin_id: &str, key: &str) -> AppResult<String> {
        let valid = !key.is_empty()
            && key.len() <= 64
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        if !valid {
            return Err(AppError::new("state.invalid_key").with("key", key));
        }
        Ok(format!("plugin:{plugin_id}:{key}"))
    }

    pub fn plugin_state_get(&self, plugin_id: &str, key: &str) -> AppResult<Option<Value>> {
        self.kv_get(&Self::plugin_state_key(plugin_id, key)?)
    }

    /// 写入插件状态；值为 null 时删除
    pub fn plugin_state_set(&self, plugin_id: &str, key: &str, value: &Value) -> AppResult<()> {
        let key = Self::plugin_state_key(plugin_id, key)?;
        if value.is_null() {
            return self.kv_delete(&key);
        }
        if value.to_string().len() > PLUGIN_STATE_LIMIT {
            return Err(AppError::new("state.too_large").with("limit", "2 MB"));
        }
        self.kv_set(&key, value)
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

    pub fn task_insert(&self, record: &TaskRecord) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO tasks (id, plugin_id, function, status, started_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![record.id, record.plugin_id, record.function, record.status, record.started_at],
        )?;
        Ok(())
    }

    pub fn task_finish(
        &self,
        id: &str,
        status: &str,
        elapsed_ms: i64,
        error_code: Option<&str>,
    ) -> AppResult<()> {
        self.conn().execute(
            "UPDATE tasks SET status = ?2, finished_at = ?3, elapsed_ms = ?4, error_code = ?5 WHERE id = ?1",
            params![id, status, now_millis(), elapsed_ms, error_code],
        )?;
        Ok(())
    }

    pub fn tasks(&self, limit: usize) -> AppResult<Vec<TaskRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, plugin_id, function, status, started_at, finished_at, elapsed_ms, error_code
             FROM tasks ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                plugin_id: row.get(1)?,
                function: row.get(2)?,
                status: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                elapsed_ms: row.get(6)?,
                error_code: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// 应用上次退出时仍在运行的任务标记为已中断
    pub fn tasks_mark_interrupted(&self) -> AppResult<usize> {
        Ok(self.conn().execute(
            "UPDATE tasks SET status = 'interrupted' WHERE status = 'running'",
            [],
        )?)
    }

    /// 只保留最近 `keep` 条任务记录，返回被删除的任务 id（用于清理日志文件）
    pub fn tasks_prune(&self, keep: usize) -> AppResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT id FROM tasks ORDER BY started_at DESC LIMIT -1 OFFSET ?1")?;
        let ids: Vec<String> = stmt
            .query_map([keep as i64], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        for id in &ids {
            conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        }
        Ok(ids)
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
#[path = "store_test.rs"]
mod tests;
