//! 本地持久化：只使用 SQLite（前端禁止使用任何浏览器存储）。

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Map, Value};

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
    r#"
    CREATE TABLE history (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        plugin_id  TEXT NOT NULL,
        function   TEXT NOT NULL,
        input      TEXT NOT NULL,
        output     TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX history_updated_at ON history (updated_at DESC);
    ALTER TABLE tasks ADD COLUMN args TEXT;
"#,
    "ALTER TABLE tasks ADD COLUMN error_params TEXT;",
    r#"
    CREATE TABLE plugin_download_outbox (
        attempt_id TEXT PRIMARY KEY,
        receipt TEXT NOT NULL,
        sha256 TEXT NOT NULL,
        size_bytes INTEGER NOT NULL CHECK(size_bytes > 0),
        expires_at INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        next_attempt_at INTEGER NOT NULL,
        attempts INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX plugin_download_outbox_due ON plugin_download_outbox(next_attempt_at);
"#,
    r#"
    CREATE TABLE agent_grants (
        plugin_id  TEXT NOT NULL,
        function   TEXT NOT NULL,
        mode       TEXT NOT NULL CHECK(mode IN ('off', 'ask', 'allow')),
        uses       TEXT,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (plugin_id, function)
    );
    CREATE TABLE agent_paths (
        path       TEXT PRIMARY KEY,
        access     TEXT NOT NULL CHECK(access IN ('read', 'write')),
        created_at INTEGER NOT NULL
    );
    ALTER TABLE history ADD COLUMN source TEXT NOT NULL DEFAULT 'app';
"#,
    // 授权按通道（命令行 / AI Agent）区分；没有记录时插件与函数默认开启
    r#"
    CREATE TABLE interface_grants (
        channel    TEXT NOT NULL CHECK(channel IN ('cli', 'agent')),
        plugin_id  TEXT NOT NULL,
        function   TEXT NOT NULL,
        mode       TEXT NOT NULL CHECK(mode IN ('off', 'ask', 'allow')),
        uses       TEXT,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (channel, plugin_id, function)
    );
    INSERT INTO interface_grants (channel, plugin_id, function, mode, uses, updated_at)
        SELECT 'agent', plugin_id, function, mode, uses, updated_at FROM agent_grants;
    DROP TABLE agent_grants;
"#,
];

/// 插件级设置使用的函数名
pub const PLUGIN_SCOPE: &str = "*";

/// 插件接口的调用设置：channel 为 cli / agent；function 为 [`PLUGIN_SCOPE`] 时是插件级开关。
/// 没有记录时插件与函数默认开启
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceGrant {
    pub channel: String,
    pub plugin_id: String,
    pub function: String,
    /// off / ask / allow
    pub mode: String,
    /// 授权时函数声明用到的权限；之后声明的权限变多时需要重新授权
    pub uses: Option<Vec<String>>,
}

/// AI Agent 可以访问的目录
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPath {
    pub path: String,
    /// read / write（write 包含读取）
    pub access: String,
}

/// 下载成功回报凭据只保存在本地 SQLite，不通过前端暴露，也不实现 Debug。
#[derive(Clone, PartialEq)]
pub struct DownloadReport {
    pub attempt_id: String,
    pub receipt: String,
    pub sha256: String,
    pub size_bytes: u64,
    /// 以下时间均为 UTC Unix 毫秒。
    pub expires_at: i64,
    pub created_at: i64,
    pub next_attempt_at: i64,
    pub attempts: u32,
}

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
    /// 与错误码一同保存，重新打开任务时仍能展示完整的本地化错误。
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub error_params: Map<String, Value>,
    /// 保存了启动参数，可以重新运行
    pub rerunnable: bool,
}

/// 一条处理记录
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub plugin_id: String,
    pub function: String,
    pub input: String,
    pub output: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// 调用来源：app / cli / agent
    pub source: String,
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
        // 桌面端与命令行（tfp、MCP 服务）可能同时写入，遇到锁时等待而不是立即失败
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
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

    /// 同一个凭据只入队一次；已有记录保留原始有效期与重试状态。
    pub fn download_report_enqueue(&self, report: &DownloadReport) -> AppResult<()> {
        let size = i64::try_from(report.size_bytes).map_err(|_| {
            AppError::new("store.failed").with("detail", "download size out of range")
        })?;
        self.conn().execute(
            "INSERT INTO plugin_download_outbox
             (attempt_id, receipt, sha256, size_bytes, expires_at, created_at, next_attempt_at, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0) ON CONFLICT(attempt_id) DO NOTHING",
            params![report.attempt_id, report.receipt, report.sha256, size,
                report.expires_at, report.created_at, report.next_attempt_at],
        )?;
        Ok(())
    }

    /// 批量清除到期凭据，并取出本轮到期的回报；断网后重启仍可继续。
    pub fn download_reports_due(&self, now: i64, limit: usize) -> AppResult<Vec<DownloadReport>> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM plugin_download_outbox WHERE expires_at <= ?1 OR created_at <= ?2",
            params![now, now.saturating_sub(5 * 24 * 60 * 60 * 1000)],
        )?;
        let mut statement = conn.prepare(
            "SELECT attempt_id, receipt, sha256, size_bytes, expires_at, created_at,
                    next_attempt_at, attempts FROM plugin_download_outbox
             WHERE next_attempt_at <= ?1 ORDER BY next_attempt_at LIMIT ?2",
        )?;
        let rows = statement.query_map(params![now, limit.min(100) as i64], |row| {
            Ok(DownloadReport {
                attempt_id: row.get(0)?,
                receipt: row.get(1)?,
                sha256: row.get(2)?,
                size_bytes: row.get::<_, i64>(3)? as u64,
                expires_at: row.get(4)?,
                created_at: row.get(5)?,
                next_attempt_at: row.get(6)?,
                attempts: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn download_report_remove(&self, attempt_id: &str) -> AppResult<()> {
        self.conn().execute(
            "DELETE FROM plugin_download_outbox WHERE attempt_id = ?1",
            [attempt_id],
        )?;
        Ok(())
    }

    pub fn download_report_retry(&self, attempt_id: &str, next_attempt_at: i64) -> AppResult<()> {
        self.conn().execute(
            "UPDATE plugin_download_outbox SET attempts = attempts + 1, next_attempt_at = ?2
             WHERE attempt_id = ?1",
            params![attempt_id, next_attempt_at],
        )?;
        Ok(())
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
        error: Option<&AppError>,
    ) -> AppResult<()> {
        self.conn().execute(
            "UPDATE tasks SET status = ?2, finished_at = ?3, elapsed_ms = ?4,
                    error_code = ?5, error_params = ?6 WHERE id = ?1",
            params![
                id,
                status,
                now_millis(),
                elapsed_ms,
                error.map(|e| e.code.as_str()),
                error.map(|e| Value::Object(e.params.clone()).to_string())
            ],
        )?;
        Ok(())
    }

    pub fn tasks(&self, limit: usize, offset: usize) -> AppResult<Vec<TaskRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, plugin_id, function, status, started_at, finished_at, elapsed_ms, error_code,
                    args IS NOT NULL, error_params
             FROM tasks ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map([limit as i64, offset as i64], |row| {
            Ok(TaskRecord {
                id: row.get(0)?,
                plugin_id: row.get(1)?,
                function: row.get(2)?,
                status: row.get(3)?,
                started_at: row.get(4)?,
                finished_at: row.get(5)?,
                elapsed_ms: row.get(6)?,
                error_code: row.get(7)?,
                error_params: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|raw| serde_json::from_str(&raw).ok())
                    .unwrap_or_default(),
                rerunnable: row.get(8)?,
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

    pub fn tasks_count(&self) -> AppResult<usize> {
        let count: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// 保存任务的启动参数（用于重新运行）
    pub fn task_set_args(&self, id: &str, args: &Value) -> AppResult<()> {
        self.conn().execute(
            "UPDATE tasks SET args = ?2 WHERE id = ?1",
            params![id, args.to_string()],
        )?;
        Ok(())
    }

    /// 任务的插件、函数与启动参数
    pub fn task_args(&self, id: &str) -> AppResult<Option<(String, String, Value)>> {
        let row: Option<(String, String, Option<String>)> = self
            .conn()
            .query_row(
                "SELECT plugin_id, function, args FROM tasks WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        Ok(row.and_then(|(plugin, function, args)| {
            let args = serde_json::from_str(&args?).ok()?;
            Some((plugin, function, args))
        }))
    }

    /// 删除已结束的任务记录，返回被删除的任务 id（用于清理日志文件）
    pub fn tasks_clear_finished(&self) -> AppResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id FROM tasks WHERE status != 'running'")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        conn.execute("DELETE FROM tasks WHERE status != 'running'", [])?;
        Ok(ids)
    }

    /// 记录一次处理；每次调用都单独成条（用户 2026-10-16 要求不合并）
    pub fn history_record(
        &self,
        plugin_id: &str,
        function: &str,
        input: &str,
        output: Option<&str>,
        now: i64,
    ) -> AppResult<i64> {
        self.history_record_from("app", plugin_id, function, input, output, now)
    }

    /// 记录一次调用并标明来源（app / cli / agent）
    pub fn history_record_from(
        &self,
        source: &str,
        plugin_id: &str,
        function: &str,
        input: &str,
        output: Option<&str>,
        now: i64,
    ) -> AppResult<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO history (plugin_id, function, input, output, created_at, updated_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![plugin_id, function, input, output, now, source],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 某个通道（cli / agent）的全部设置
    pub fn interface_grants(&self, channel: &str) -> AppResult<Vec<InterfaceGrant>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT channel, plugin_id, function, mode, uses FROM interface_grants
             WHERE channel = ?1 ORDER BY plugin_id, function",
        )?;
        let rows = stmt.query_map([channel], |row| {
            let uses: Option<String> = row.get(4)?;
            Ok(InterfaceGrant {
                channel: row.get(0)?,
                plugin_id: row.get(1)?,
                function: row.get(2)?,
                mode: row.get(3)?,
                uses: uses.and_then(|u| serde_json::from_str(&u).ok()),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn interface_grant_set(&self, grant: &InterfaceGrant) -> AppResult<()> {
        let uses = grant
            .uses
            .as_ref()
            .map(|u| serde_json::to_string(u).unwrap_or_default());
        self.conn().execute(
            "INSERT INTO interface_grants (channel, plugin_id, function, mode, uses, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(channel, plugin_id, function) DO UPDATE
             SET mode = excluded.mode, uses = excluded.uses, updated_at = excluded.updated_at",
            params![
                grant.channel,
                grant.plugin_id,
                grant.function,
                grant.mode,
                uses,
                now_millis()
            ],
        )?;
        Ok(())
    }

    pub fn interface_grant_remove(
        &self,
        channel: &str,
        plugin_id: &str,
        function: &str,
    ) -> AppResult<()> {
        self.conn().execute(
            "DELETE FROM interface_grants WHERE channel = ?1 AND plugin_id = ?2 AND function = ?3",
            params![channel, plugin_id, function],
        )?;
        Ok(())
    }

    pub fn agent_paths(&self) -> AppResult<Vec<AgentPath>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT path, access FROM agent_paths ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            Ok(AgentPath {
                path: row.get(0)?,
                access: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn agent_path_set(&self, path: &AgentPath) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO agent_paths (path, access, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET access = excluded.access",
            params![path.path, path.access, now_millis()],
        )?;
        Ok(())
    }

    pub fn agent_path_remove(&self, path: &str) -> AppResult<()> {
        self.conn()
            .execute("DELETE FROM agent_paths WHERE path = ?1", [path])?;
        Ok(())
    }

    /// 按更新时间倒序列出记录；query 匹配输入、输出或插件 id
    pub fn history_list(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> AppResult<Vec<HistoryEntry>> {
        let conn = self.conn();
        let pattern = format!(
            "%{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut stmt = conn.prepare(
            "SELECT id, plugin_id, function, input, output, created_at, updated_at, source FROM history
             WHERE ?1 = '' OR input LIKE ?2 ESCAPE '\\' OR output LIKE ?2 ESCAPE '\\' OR plugin_id LIKE ?2 ESCAPE '\\'
             ORDER BY updated_at DESC, id DESC LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt.query_map(
            params![query, pattern, limit as i64, offset as i64],
            |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    plugin_id: row.get(1)?,
                    function: row.get(2)?,
                    input: row.get(3)?,
                    output: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    source: row.get(7)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn history_count(&self) -> AppResult<usize> {
        let count: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn history_delete(&self, id: i64) -> AppResult<()> {
        self.conn()
            .execute("DELETE FROM history WHERE id = ?1", [id])?;
        Ok(())
    }

    /// 删除记录；plugin_id 为 None 时清空全部
    pub fn history_clear(&self, plugin_id: Option<&str>) -> AppResult<usize> {
        Ok(match plugin_id {
            Some(id) => self
                .conn()
                .execute("DELETE FROM history WHERE plugin_id = ?1", [id])?,
            None => self.conn().execute("DELETE FROM history", [])?,
        })
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
