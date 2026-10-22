//! 官方成功下载统计：后台持久化回报与前端只读查询。

use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use reqwest::blocking::Client;
use serde_json::{Value, json};
use tf_core::store::{DownloadReport, now_millis};
use tf_core::{AppError, AppResult, Store};
use url::Url;

use crate::plugin_download::{STATS_BASE, client};

const MAX_STATS: u64 = 2 * 1024 * 1024;

pub(crate) fn init(store: Arc<Store>) {
    std::thread::spawn(move || {
        loop {
            match client(Duration::from_secs(15)) {
                Ok(client) => {
                    if let Err(error) = flush(&client, &store, STATS_BASE, now_millis()) {
                        log::warn!("plugin download confirmation queue: {}", error.code);
                    }
                }
                Err(error) => log::warn!("plugin download confirmation client: {}", error.code),
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
}

fn retry_after(value: Option<&reqwest::header::HeaderValue>, now: i64) -> Option<i64> {
    let text = value?.to_str().ok()?;
    if let Ok(seconds) = text.parse::<u64>() {
        return Some(
            now.saturating_add(
                i64::try_from(seconds)
                    .unwrap_or(i64::MAX)
                    .saturating_mul(1000),
            ),
        );
    }
    let date = httpdate::parse_http_date(text)
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?;
    Some(i64::try_from(date.as_millis()).unwrap_or(i64::MAX).max(now))
}

fn next_retry(report: &DownloadReport, now: i64, server_after: Option<i64>) -> i64 {
    let base = (30_000_i64 * (1_i64 << report.attempts.min(7))).min(3_600_000);
    let random = uuid::Uuid::new_v4().as_u128() as u64;
    let jitter = (random % (base as u64 / 4 + 1)) as i64;
    now.saturating_add(base)
        .saturating_add(jitter)
        .max(server_after.unwrap_or(now))
}

fn flush(client: &Client, store: &Store, base: &str, now: i64) -> AppResult<()> {
    for report in store.download_reports_due(now, 16)? {
        // 即使本地数据库被手工修改，也不让不合法记录触发任意地址或路径请求。
        if report.attempt_id.len() != 32
            || !report.attempt_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            store.download_report_remove(&report.attempt_id)?;
            continue;
        }
        let response = client
            .post(format!(
                "{base}/v1/downloads/{}/complete",
                report.attempt_id
            ))
            .bearer_auth(&report.receipt)
            .json(&json!({"sha256": report.sha256, "size": report.size_bytes}))
            .send();
        match response {
            Ok(response) if response.status().is_success() => {
                store.download_report_remove(&report.attempt_id)?
            }
            Ok(response)
                if response.status().is_server_error()
                    || matches!(response.status().as_u16(), 408 | 429) =>
            {
                let after = retry_after(response.headers().get(reqwest::header::RETRY_AFTER), now);
                store.download_report_retry(&report.attempt_id, next_retry(&report, now, after))?;
            }
            // 过期、撤销、元数据冲突、错误鉴权及重定向都不继续发送凭据。
            Ok(_) => store.download_report_remove(&report.attempt_id)?,
            Err(_) => {
                store.download_report_retry(&report.attempt_id, next_retry(&report, now, None))?
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn plugin_download_stats(ids: Vec<String>) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || query(&ids))
        .await
        .map_err(|error| AppError::new("plugin.crashed").with("detail", error.to_string()))?
}

fn query(ids: &[String]) -> AppResult<Value> {
    if ids.is_empty() || ids.len() > 100 || ids.iter().any(|id| !tf_package::valid_id(id)) {
        return Err(AppError::new("market.invalid_download"));
    }
    let mut url = Url::parse(&format!("{STATS_BASE}/v1/stats"))
        .map_err(|_| AppError::new("market.invalid_download"))?;
    url.query_pairs_mut().append_pair("ids", &ids.join(","));
    let response = client(Duration::from_secs(15))?
        .get(url)
        .send()
        .map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(unavailable());
    }
    if response.content_length().is_some_and(|len| len > MAX_STATS) {
        return Err(unavailable());
    }
    let mut body = Vec::new();
    response
        .take(MAX_STATS + 1)
        .read_to_end(&mut body)
        .map_err(|_| unavailable())?;
    if body.len() as u64 > MAX_STATS {
        return Err(unavailable());
    }
    let value: Value = serde_json::from_slice(&body).map_err(|_| unavailable())?;
    if !value.get("plugins").is_some_and(Value::is_array) {
        return Err(unavailable());
    }
    Ok(value)
}

fn unavailable() -> AppError {
    AppError::new("market.stats_unavailable")
}

#[cfg(test)]
#[path = "plugin_stats_test.rs"]
mod tests;
