//! 从内置工具迁出为在线插件的工具：启动时（以及每次成功读取插件索引后）在后台自动安装，
//! 让老用户升级后常用工具不会消失。
//!
//! 已处理过的插件 id 记录在 `installed/.migrated.json`；安装成功或用户卸载后都会记录，
//! 之后不再自动安装。索引读取失败（如离线）或插件尚未发布时不记录，下次再试。

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

/// 已迁出的内置工具 id（保持原 id，用户数据与历史记录随之保留）
pub const MIGRATED: &[&str] = &[
    "org.devlive.toolforge.date-calculator",
    "org.devlive.toolforge.semver",
    "org.devlive.toolforge.sql-formatter",
    "org.devlive.toolforge.timestamp",
];

pub const MARKER: &str = ".migrated.json";

/// 读取已处理的 id；文件缺失或损坏视为空
pub fn done(root: &Path) -> BTreeSet<String> {
    std::fs::read(root.join(MARKER))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

/// 追加记录已处理的 id；仅对迁出列表中的 id 生效
pub fn mark(root: &Path, id: &str) -> std::io::Result<()> {
    if !MIGRATED.contains(&id) {
        return Ok(());
    }
    let mut ids = done(root);
    if ids.insert(id.to_owned()) {
        std::fs::create_dir_all(root)?;
        let raw = serde_json::to_vec_pretty(&ids).map_err(std::io::Error::other)?;
        std::fs::write(root.join(MARKER), raw)?;
    }
    Ok(())
}

/// 尚未处理也未安装的迁出 id
pub fn pending(done: &BTreeSet<String>, installed: &BTreeSet<String>) -> Vec<&'static str> {
    MIGRATED
        .iter()
        .copied()
        .filter(|id| !done.contains(*id) && !installed.contains(*id))
        .collect()
}

/// 数字段逐段比较；非数字段按 0 处理
fn at_least(version: &str, min: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parts(version), parts(min));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    true
}

/// 自动安装的下载目标
#[derive(Debug, PartialEq, Eq)]
pub struct Download {
    pub id: String,
    pub url: String,
    pub sha256: String,
}

/// 从索引中挑出待安装的条目：只装官方签名、且当前应用版本满足 minAppVersion 的插件
pub fn downloads(index: &Value, pending: &[&str], app_version: &str) -> Vec<Download> {
    let Some(entries) = index.get("plugins").and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?;
            if !pending.contains(&id) || entry.get("signed").and_then(Value::as_bool) != Some(true)
            {
                return None;
            }
            if let Some(min) = entry.get("minAppVersion").and_then(Value::as_str)
                && !at_least(app_version, min)
            {
                return None;
            }
            Some(Download {
                id: id.to_owned(),
                url: entry.get("url")?.as_str()?.to_owned(),
                sha256: entry.get("sha256")?.as_str()?.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "migration_test.rs"]
mod tests;
