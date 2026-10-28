//! 在线插件各版本的发布时间，用于市场的「最新发布 / 最近更新」排序与筛选。
//!
//! - 已发布版本的时间以旧发布目录中的 `publishedAt` 为准，永不改写。
//! - 旧目录没有记录的版本，从 git 历史中取该版本号第一次出现在 manifest 里的提交时间
//!   （推送到 dev 即发布，提交时间接近发布时间）。
//! - 本次新增的版本用本次发布源提交的时间。
//! - 时间统一为 UTC 的 RFC 3339（`YYYY-MM-DDTHH:MM:SSZ`），可直接按字符串比较先后。

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// (插件 id, 版本) → 发布时间
pub type VersionDates = BTreeMap<(String, String), String>;

#[derive(Debug, Default, Clone)]
pub struct ReleaseDates {
    /// git 历史推断的各版本时间
    pub versions: VersionDates,
    /// 本次发布源提交的时间（新版本使用）
    pub now: Option<String>,
}

const DATE_FORMAT: &str = "--date=format-local:%Y-%m-%dT%H:%M:%SZ";

fn git_utc(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .env("TZ", "UTC")
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 读取 market/*/manifest.json：路径 → 插件 id
fn manifest_ids(root: &Path) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(root.join("market")) else {
        return ids;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path().join("manifest.json");
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(id) = value["id"].as_str() {
            let rel = format!(
                "market/{}/manifest.json",
                entry.file_name().to_string_lossy()
            );
            ids.insert(rel, id.to_owned());
        }
    }
    ids
}

/// 从仓库历史推断各版本时间；任何 git 失败都返回空结果，不阻止发布
pub fn from_git(root: &Path, source_commit: &str) -> ReleaseDates {
    let ids = manifest_ids(root);
    let log = git_utc(
        root,
        &[
            "log",
            "--reverse",
            "--format=@@commit %cd",
            DATE_FORMAT,
            "-p",
            "--unified=0",
            "--no-color",
            "--",
            "market/*/manifest.json",
        ],
    )
    .unwrap_or_default();
    let now = git_utc(
        root,
        &["log", "-1", "--format=%cd", DATE_FORMAT, source_commit],
    )
    .ok()
    .map(|s| s.trim().to_owned())
    .filter(|s| is_utc_timestamp(s));
    ReleaseDates {
        versions: parse_log(&log, &ids),
        now,
    }
}

/// 解析 `git log --reverse -p` 输出：每个 manifest 中新增的 `"version": "x"` 行，
/// 取最早出现的提交时间
pub fn parse_log(log: &str, ids: &BTreeMap<String, String>) -> VersionDates {
    let mut dates = VersionDates::new();
    let mut date: Option<&str> = None;
    let mut file: Option<&str> = None;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("@@commit ") {
            date = Some(rest.trim()).filter(|d| is_utc_timestamp(d));
            file = None;
        } else if let Some(path) = line.strip_prefix("+++ b/") {
            file = Some(path.trim());
        } else if line.starts_with("+++ ") {
            file = None;
        } else if let Some(added) = line.strip_prefix('+')
            && let (Some(date), Some(file)) = (date, file)
            && let Some(id) = ids.get(file)
            && let Some(version) = version_of(added)
        {
            dates
                .entry((id.clone(), version))
                .or_insert_with(|| date.to_owned());
        }
    }
    dates
}

/// 取出一行 JSON 里的 `"version": "x"` 值
fn version_of(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("\"version\"")?.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

pub fn is_utc_timestamp(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && b.iter()
            .enumerate()
            .all(|(i, c)| [4, 7, 10, 13, 16, 19].contains(&i) || c.is_ascii_digit())
}

#[cfg(test)]
#[path = "release_dates_test.rs"]
mod tests;
