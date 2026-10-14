//! 统计数据的 SQLite 存储：按天累加，查询时按日期范围汇总。

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

use crate::stats::Pending;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS keys (
    day TEXT NOT NULL, code TEXT NOT NULL, count INTEGER NOT NULL,
    PRIMARY KEY (day, code)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS combos (
    day TEXT NOT NULL, combo TEXT NOT NULL, count INTEGER NOT NULL,
    PRIMARY KEY (day, combo)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS buttons (
    day TEXT NOT NULL, button TEXT NOT NULL, count INTEGER NOT NULL,
    PRIMARY KEY (day, button)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS hours (
    day TEXT NOT NULL, hour INTEGER NOT NULL,
    keys INTEGER NOT NULL, clicks INTEGER NOT NULL, scroll REAL NOT NULL,
    distance REAL NOT NULL, active INTEGER NOT NULL,
    PRIMARY KEY (day, hour)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS clicks (
    day TEXT NOT NULL, display TEXT NOT NULL, col INTEGER NOT NULL, row INTEGER NOT NULL,
    count INTEGER NOT NULL,
    PRIMARY KEY (day, display, col, row)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS displays (
    id TEXT PRIMARY KEY, x REAL NOT NULL, y REAL NOT NULL,
    width REAL NOT NULL, height REAL NOT NULL, cols INTEGER NOT NULL, rows INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS apps (
    day TEXT NOT NULL, app TEXT NOT NULL, name TEXT NOT NULL,
    active INTEGER NOT NULL, keys INTEGER NOT NULL, clicks INTEGER NOT NULL,
    PRIMARY KEY (day, app)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

/// 统计数据表（清空时删除这些表的内容，保留设置）
const DATA_TABLES: [&str; 7] = [
    "keys", "combos", "buttons", "hours", "clicks", "displays", "apps",
];

fn db_error(err: rusqlite::Error) -> PluginError {
    PluginError::new("input_stats.database").with("detail", err.to_string())
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub keys: u64,
    pub clicks: u64,
    pub scroll: f64,
    /// 毫米
    pub distance: f64,
    /// 秒
    pub active: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Count {
    pub name: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayRow {
    pub day: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppRow {
    pub id: String,
    pub name: String,
    pub active: u64,
    pub keys: u64,
    pub clicks: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub from: String,
    pub to: String,
    pub totals: Totals,
    /// 每个键的次数，按次数从高到低
    pub keys: Vec<Count>,
    pub combos: Vec<Count>,
    pub buttons: Vec<Count>,
    /// 0–23 点，长度固定为 24
    pub hours: Vec<Totals>,
    /// 范围内有数据的日期
    pub days: Vec<DayRow>,
    pub apps: Vec<AppRow>,
    /// 最早有数据的日期
    pub first_day: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayMap {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub cols: u32,
    pub rows: u32,
    /// `[列, 行, 次数]`
    pub cells: Vec<[u64; 3]>,
    pub max: u64,
    pub total: u64,
}

/// 组合键最多返回的条数
const COMBO_LIMIT: usize = 40;
/// 应用最多返回的条数
const APP_LIMIT: usize = 100;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> PluginResult<Self> {
        Self::init(Connection::open(path).map_err(db_error)?)
    }

    #[cfg(test)]
    pub fn memory() -> PluginResult<Self> {
        Self::init(Connection::open_in_memory().map_err(db_error)?)
    }

    fn init(conn: Connection) -> PluginResult<Self> {
        conn.execute_batch(&format!("PRAGMA journal_mode = WAL; {SCHEMA}"))
            .map_err(db_error)?;
        Ok(Self { conn })
    }

    pub fn setting(&self, key: &str) -> PluginResult<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(db_error)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> PluginResult<()> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map_err(db_error)?;
        Ok(())
    }

    /// 把内存中的计数累加进数据库（单个事务）
    pub fn flush(&mut self, p: &Pending) -> PluginResult<()> {
        if p.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO keys (day, code, count) VALUES (?1, ?2, ?3)
                     ON CONFLICT DO UPDATE SET count = count + excluded.count",
                )
                .map_err(db_error)?;
            for ((day, code), count) in &p.keys {
                stmt.execute(params![day, code, count]).map_err(db_error)?;
            }
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO combos (day, combo, count) VALUES (?1, ?2, ?3)
                     ON CONFLICT DO UPDATE SET count = count + excluded.count",
                )
                .map_err(db_error)?;
            for ((day, combo), count) in &p.combos {
                stmt.execute(params![day, combo, count]).map_err(db_error)?;
            }
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO buttons (day, button, count) VALUES (?1, ?2, ?3)
                     ON CONFLICT DO UPDATE SET count = count + excluded.count",
                )
                .map_err(db_error)?;
            for ((day, button), count) in &p.buttons {
                stmt.execute(params![day, button, count])
                    .map_err(db_error)?;
            }
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO hours (day, hour, keys, clicks, scroll, distance, active)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT DO UPDATE SET keys = keys + excluded.keys,
                        clicks = clicks + excluded.clicks, scroll = scroll + excluded.scroll,
                        distance = distance + excluded.distance,
                        active = active + excluded.active",
                )
                .map_err(db_error)?;
            for ((day, hour), h) in &p.hours {
                stmt.execute(params![
                    day, hour, h.keys, h.clicks, h.scroll, h.distance, h.active
                ])
                .map_err(db_error)?;
            }
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO clicks (day, display, col, row, count) VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT DO UPDATE SET count = count + excluded.count",
                )
                .map_err(db_error)?;
            for ((day, display, col, row), count) in &p.clicks {
                stmt.execute(params![day, display, col, row, count])
                    .map_err(db_error)?;
            }
            let mut stmt = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO displays (id, x, y, width, height, cols, rows)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(db_error)?;
            for (id, d) in &p.displays {
                let (cols, rows) = d.grid();
                stmt.execute(params![id, d.x, d.y, d.width, d.height, cols, rows])
                    .map_err(db_error)?;
            }
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO apps (day, app, name, active, keys, clicks)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT DO UPDATE SET name = excluded.name,
                        active = active + excluded.active, keys = keys + excluded.keys,
                        clicks = clicks + excluded.clicks",
                )
                .map_err(db_error)?;
            for ((day, app), a) in &p.apps {
                stmt.execute(params![day, app, a.name, a.active, a.keys, a.clicks])
                    .map_err(db_error)?;
            }
        }
        tx.commit().map_err(db_error)
    }

    fn counts(&self, sql: &str, from: &str, to: &str) -> PluginResult<Vec<Count>> {
        let mut stmt = self.conn.prepare(sql).map_err(db_error)?;
        let rows = stmt
            .query_map([from, to], |row| {
                Ok(Count {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            })
            .map_err(db_error)?;
        rows.collect::<Result<_, _>>().map_err(db_error)
    }

    /// 汇总 `from..=to`（`YYYY-MM-DD`）范围内的数据
    pub fn summary(&self, from: &str, to: &str) -> PluginResult<Summary> {
        let keys = self.counts(
            "SELECT code, SUM(count) AS n FROM keys WHERE day BETWEEN ?1 AND ?2
             GROUP BY code ORDER BY n DESC, code",
            from,
            to,
        )?;
        let mut combos = self.counts(
            "SELECT combo, SUM(count) AS n FROM combos WHERE day BETWEEN ?1 AND ?2
             GROUP BY combo ORDER BY n DESC, combo",
            from,
            to,
        )?;
        combos.truncate(COMBO_LIMIT);
        let buttons = self.counts(
            "SELECT button, SUM(count) AS n FROM buttons WHERE day BETWEEN ?1 AND ?2
             GROUP BY button ORDER BY n DESC, button",
            from,
            to,
        )?;

        let mut hours = vec![Totals::default(); 24];
        let mut by_day: Vec<DayRow> = Vec::new();
        let mut stmt = self
            .conn
            .prepare(
                "SELECT day, hour, keys, clicks, scroll, distance, active FROM hours
                 WHERE day BETWEEN ?1 AND ?2 ORDER BY day, hour",
            )
            .map_err(db_error)?;
        let rows = stmt
            .query_map([from, to], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u8>(1)?,
                    Totals {
                        keys: row.get(2)?,
                        clicks: row.get(3)?,
                        scroll: row.get(4)?,
                        distance: row.get(5)?,
                        active: row.get(6)?,
                    },
                ))
            })
            .map_err(db_error)?;
        let mut totals = Totals::default();
        for row in rows {
            let (day, hour, t) = row.map_err(db_error)?;
            add(&mut totals, &t);
            if let Some(slot) = hours.get_mut(usize::from(hour)) {
                add(slot, &t);
            }
            match by_day.last_mut() {
                Some(last) if last.day == day => add(&mut last.totals, &t),
                _ => by_day.push(DayRow { day, totals: t }),
            }
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT app, MAX(name), SUM(active) AS a, SUM(keys), SUM(clicks) FROM apps
                 WHERE day BETWEEN ?1 AND ?2 GROUP BY app ORDER BY a DESC, app LIMIT ?3",
            )
            .map_err(db_error)?;
        let apps = stmt
            .query_map(params![from, to, APP_LIMIT], |row| {
                Ok(AppRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    active: row.get(2)?,
                    keys: row.get(3)?,
                    clicks: row.get(4)?,
                })
            })
            .map_err(db_error)?
            .collect::<Result<_, _>>()
            .map_err(db_error)?;
        let first_day = self
            .conn
            .query_row("SELECT MIN(day) FROM hours", [], |row| row.get(0))
            .map_err(db_error)?;

        Ok(Summary {
            from: from.to_owned(),
            to: to.to_owned(),
            totals,
            keys,
            combos,
            buttons,
            hours,
            days: by_day,
            apps,
            first_day,
        })
    }

    /// 范围内各显示器的点击热力格
    pub fn click_map(&self, from: &str, to: &str) -> PluginResult<Vec<DisplayMap>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.display, d.x, d.y, d.width, d.height, d.cols, d.rows,
                        c.col, c.row, SUM(c.count)
                 FROM clicks c JOIN displays d ON d.id = c.display
                 WHERE c.day BETWEEN ?1 AND ?2
                 GROUP BY c.display, c.col, c.row ORDER BY c.display, c.row, c.col",
            )
            .map_err(db_error)?;
        let rows = stmt
            .query_map([from, to], |row| {
                Ok((
                    DisplayMap {
                        id: row.get(0)?,
                        x: row.get(1)?,
                        y: row.get(2)?,
                        width: row.get(3)?,
                        height: row.get(4)?,
                        cols: row.get(5)?,
                        rows: row.get(6)?,
                        cells: Vec::new(),
                        max: 0,
                        total: 0,
                    },
                    [row.get::<_, u64>(7)?, row.get(8)?, row.get(9)?],
                ))
            })
            .map_err(db_error)?;
        let mut maps: Vec<DisplayMap> = Vec::new();
        for row in rows {
            let (display, cell) = row.map_err(db_error)?;
            if maps.last().is_none_or(|m| m.id != display.id) {
                maps.push(display);
            }
            let map = maps.last_mut().expect("pushed above");
            map.max = map.max.max(cell[2]);
            map.total += cell[2];
            map.cells.push(cell);
        }
        Ok(maps)
    }

    /// 出现过的应用（用于选择排除名单），按累计活跃时间排序
    pub fn known_apps(&self) -> PluginResult<Vec<AppRow>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT app, MAX(name), SUM(active) AS a, SUM(keys), SUM(clicks) FROM apps
                 GROUP BY app ORDER BY a DESC, app LIMIT ?1",
            )
            .map_err(db_error)?;
        stmt.query_map([APP_LIMIT * 2], |row| {
            Ok(AppRow {
                id: row.get(0)?,
                name: row.get(1)?,
                active: row.get(2)?,
                keys: row.get(3)?,
                clicks: row.get(4)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)
    }

    /// 应用的显示名称（排除名单中的应用可能已经没有统计数据）
    pub fn app_names(&self) -> PluginResult<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT app, MAX(name) FROM apps GROUP BY app")
            .map_err(db_error)?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(db_error)?
            .collect::<Result<_, _>>()
            .map_err(db_error)
    }

    /// 删除全部统计数据（保留设置）
    pub fn clear(&mut self) -> PluginResult<()> {
        let tx = self.conn.transaction().map_err(db_error)?;
        for table in DATA_TABLES {
            tx.execute(&format!("DELETE FROM {table}"), [])
                .map_err(db_error)?;
        }
        tx.commit().map_err(db_error)?;
        self.conn.execute_batch("VACUUM").map_err(db_error)
    }
}

fn add(into: &mut Totals, t: &Totals) {
    into.keys += t.keys;
    into.clicks += t.clicks;
    into.scroll += t.scroll;
    into.distance += t.distance;
    into.active += t.active;
}

#[cfg(test)]
#[path = "store_test.rs"]
mod tests;
