//! 输入统计插件后端：在后台统计每天的按键、鼠标点击、移动距离与应用使用时长。
//!
//! 只累加次数，不记录按键顺序与输入内容；数据只保存在本机。

mod collector;
mod keymap;
mod platform;
mod stats;
mod store;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use jiff::ToSpan;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ServiceContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use collector::{Collector, Shared};
use store::{AppRow, Store, Totals};

const MANIFEST: &str = include_str!("../../manifest.json");
const DB_FILE: &str = "input-stats.db";
const ENABLED: &str = "enabled";
const EXCLUDED: &str = "excluded";

/// 查询的日期范围：预设或自定义的 `from..=to`（`YYYY-MM-DD`）
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RangeArgs {
    #[serde(default)]
    range: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

fn parse_date(text: &str) -> PluginResult<Date> {
    text.parse()
        .map_err(|_| PluginError::new("input_stats.invalid_date").with("date", text))
}

impl RangeArgs {
    fn resolve(&self, today: Date) -> PluginResult<(String, String)> {
        if let (Some(from), Some(to)) = (&self.from, &self.to) {
            let (from, to) = (parse_date(from)?, parse_date(to)?);
            let (from, to) = if from <= to { (from, to) } else { (to, from) };
            return Ok((from.to_string(), to.to_string()));
        }
        let days_back = |n: i64| {
            today
                .checked_sub(n.days())
                .map_err(|_| PluginError::new("input_stats.invalid_date"))
        };
        let (from, to) = match self.range.as_deref().unwrap_or("today") {
            "today" => (today, today),
            "yesterday" => {
                let day = days_back(1)?;
                (day, day)
            }
            "week" => (days_back(6)?, today),
            "month" => (days_back(29)?, today),
            "all" => (Date::constant(1970, 1, 1), today),
            other => {
                return Err(PluginError::new("input_stats.invalid_range").with("range", other));
            }
        };
        Ok((from.to_string(), to.to_string()))
    }
}

fn today() -> Date {
    jiff::Zoned::now().date()
}

#[derive(Deserialize)]
struct EnabledArgs {
    enabled: bool,
}

#[derive(Deserialize)]
struct ExcludedArgs {
    apps: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppRef {
    id: String,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusOut {
    os: &'static str,
    /// 当前环境不支持时的原因码（如 wayland）
    unsupported: Option<&'static str>,
    permission: &'static str,
    enabled: bool,
    capturing: bool,
    error: Option<String>,
    app: Option<AppRef>,
    today: Totals,
    excluded: Vec<AppRef>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppsOut {
    apps: Vec<AppRow>,
    excluded: Vec<AppRef>,
}

pub struct InputStats {
    manifest: Manifest,
    /// 服务数据；None 表示未启动，停止时清空以释放数据库连接与文件句柄
    shared: Mutex<Option<Arc<Shared>>>,
    collector: Mutex<Option<Collector>>,
}

impl Default for InputStats {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            shared: Mutex::new(None),
            collector: Mutex::new(None),
        }
    }
}

impl InputStats {
    fn shared(&self) -> PluginResult<Arc<Shared>> {
        self.shared
            .lock()
            .expect("shared poisoned")
            .clone()
            .ok_or_else(|| PluginError::new("input_stats.not_started"))
    }

    fn excluded(&self) -> PluginResult<Vec<String>> {
        let shared = self.shared()?;
        let raw = shared
            .store
            .lock()
            .expect("store poisoned")
            .setting(EXCLUDED)?;
        Ok(raw
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default())
    }

    fn excluded_refs(&self) -> PluginResult<Vec<AppRef>> {
        let shared = self.shared()?;
        let names = shared.store.lock().expect("store poisoned").app_names()?;
        Ok(self
            .excluded()?
            .into_iter()
            .map(|id| AppRef {
                name: names.get(&id).cloned().unwrap_or_else(|| id.clone()),
                id,
            })
            .collect())
    }

    fn running(&self) -> bool {
        self.collector.lock().expect("collector poisoned").is_some()
    }

    /// 开始或停止采集；不支持的环境下不会启动
    fn apply_enabled(&self, enabled: bool) -> PluginResult<()> {
        let shared = self.shared()?;
        let mut collector = self.collector.lock().expect("collector poisoned");
        if enabled && platform::support().is_ok() {
            if collector.is_none() {
                shared.set_excluded(self.excluded()?.into_iter().collect());
                *collector = Some(Collector::start(shared));
            }
        } else if let Some(running) = collector.take() {
            running.stop();
        }
        Ok(())
    }

    fn status(&self) -> PluginResult<StatusOut> {
        let shared = self.shared()?;
        shared.flush()?;
        let store = shared.store.lock().expect("store poisoned");
        let enabled = store.setting(ENABLED)?.as_deref() == Some("1");
        let day = today().to_string();
        let today = store.summary(&day, &day)?.totals;
        // 释放数据库锁后再做不需要数据库的查询
        drop(store);
        let running = self.running();
        let status = shared.status.lock().expect("status poisoned").clone();
        Ok(StatusOut {
            os: std::env::consts::OS,
            unsupported: platform::support().err(),
            permission: platform::permission().name(),
            enabled,
            capturing: running && status.capturing,
            error: if running { status.error } else { None },
            app: status.app.map(|app| AppRef {
                id: app.id,
                name: app.name,
            }),
            today,
            excluded: self.excluded_refs()?,
        })
    }
}

impl ToolPlugin for InputStats {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn start_service(&self, ctx: Arc<dyn ServiceContext>) -> PluginResult<()> {
        let mut shared = self.shared.lock().expect("shared poisoned");
        if shared.is_some() {
            // 已启动时幂等，不重复打开数据库
            return Ok(());
        }
        let store = Store::open(&ctx.data_dir().join(DB_FILE))?;
        let enabled = store.setting(ENABLED)?.as_deref() == Some("1");
        *shared = Some(Arc::new(Shared::new(store)));
        drop(shared);
        self.apply_enabled(enabled)
    }

    fn stop_service(&self) {
        if let Some(collector) = self.collector.lock().expect("collector poisoned").take() {
            collector.stop();
        }
        // 最后一次落盘后丢弃共享状态，关闭数据库连接并释放文件句柄
        if let Some(shared) = self.shared.lock().expect("shared poisoned").take() {
            let _ = shared.flush();
        }
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "status" => to_value(self.status()?),
            "set_enabled" => {
                let args: EnabledArgs = parse_args(args)?;
                let shared = self.shared()?;
                shared
                    .store
                    .lock()
                    .expect("store poisoned")
                    .set_setting(ENABLED, if args.enabled { "1" } else { "0" })?;
                self.apply_enabled(args.enabled)?;
                to_value(self.status()?)
            }
            "request_permission" => {
                platform::request_permission();
                to_value(self.status()?)
            }
            "summary" => {
                let args: RangeArgs = parse_args(args)?;
                let (from, to) = args.resolve(today())?;
                let shared = self.shared()?;
                shared.flush()?;
                to_value(
                    shared
                        .store
                        .lock()
                        .expect("store poisoned")
                        .summary(&from, &to)?,
                )
            }
            "click_map" => {
                let args: RangeArgs = parse_args(args)?;
                let (from, to) = args.resolve(today())?;
                let shared = self.shared()?;
                shared.flush()?;
                to_value(
                    shared
                        .store
                        .lock()
                        .expect("store poisoned")
                        .click_map(&from, &to)?,
                )
            }
            "apps" => {
                let shared = self.shared()?;
                shared.flush()?;
                // 先取完应用列表再查排除名单：两者都要锁住数据库
                let apps = shared.store.lock().expect("store poisoned").known_apps()?;
                to_value(AppsOut {
                    apps,
                    excluded: self.excluded_refs()?,
                })
            }
            "set_excluded" => {
                let args: ExcludedArgs = parse_args(args)?;
                let unique: Vec<String> = args
                    .apps
                    .into_iter()
                    .filter(|id| !id.is_empty())
                    .collect::<indexmap::IndexSet<_>>()
                    .into_iter()
                    .collect();
                let raw = serde_json::to_string(&unique).map_err(|e| {
                    PluginError::new("plugin.serialize_failed").with("detail", e.to_string())
                })?;
                let shared = self.shared()?;
                shared
                    .store
                    .lock()
                    .expect("store poisoned")
                    .set_setting(EXCLUDED, &raw)?;
                shared.set_excluded(unique.into_iter().collect::<HashSet<_>>());
                to_value(self.excluded_refs()?)
            }
            "clear" => {
                let shared = self.shared()?;
                // 先丢弃内存中尚未写入的计数，再清空数据库
                let _ = shared.recorder.lock().expect("recorder poisoned").take();
                shared.store.lock().expect("store poisoned").clear()?;
                to_value(self.status()?)
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
