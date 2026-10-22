//! 官方下载统一经过统计服务，302 回报凭据仅留在 Rust/SQLite 中。

use std::io::Read;
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT_ENCODING, LOCATION};
use tf_core::store::{DownloadReport, now_millis};
use tf_core::{AppError, AppResult, Store, resources};
use url::Url;

use crate::installed::package_error;

pub(crate) const STATS_BASE: &str = "https://stats.toolforge.devlive.org";
const MAX_PACKAGE: u64 = tf_package::MAX_PACKAGE as u64;
const MAX_RECEIPT_AGE: i64 = 7 * 24 * 60 * 60 * 1000;

pub(crate) struct Expected<'a> {
    pub id: &'a str,
    pub version: &'a str,
    pub sha256: &'a str,
    pub size: u64,
}

pub(crate) fn client(timeout: Duration) -> AppResult<Client> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    Client::builder()
        .user_agent(concat!("ToolForge/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .build()
        .map_err(|_| network())
}

fn network() -> AppError {
    // reqwest 的完整错误可能携带 URL 查询串，不传入日志或前端。
    AppError::new("resource.network")
        .with("host", "plugins.toolforge.devlive.org")
        .with("detail", "plugin download request failed")
}

fn invalid() -> AppError {
    AppError::new("market.invalid_download")
}

fn metadata_error() -> AppError {
    AppError::new("market.download_metadata_mismatch")
}

fn clean_https(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.port_or_known_default() == Some(443)
}

pub(crate) fn is_stats_url(url: &Url) -> bool {
    clean_https(url) && url.host_str() == Some("stats.toolforge.devlive.org")
}

fn allowed_asset(url: &Url) -> bool {
    #[cfg(test)]
    if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") {
        return url.username().is_empty() && url.password().is_none() && url.fragment().is_none();
    }
    clean_https(url)
        && matches!(
            url.host_str(),
            Some(
                "plugins.toolforge.devlive.org"
                    | "toolforge-1258585472.cos.ap-singapore.myqcloud.com"
            )
        )
}

fn validate_expected(expected: &Expected<'_>) -> AppResult<()> {
    if !tf_package::valid_id(expected.id)
        || expected.version.is_empty()
        || expected.version.len() > 64
        || !expected
            .version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".+-".contains(&b))
        || expected.sha256.len() != 64
        || !expected.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || expected.size == 0
        || expected.size > MAX_PACKAGE
    {
        return Err(metadata_error());
    }
    Ok(())
}

fn validate_api(url: &Url, expected: &Expected<'_>) -> AppResult<()> {
    validate_expected(expected)?;
    let path = format!(
        "/v1/plugins/{}/versions/{}/download",
        expected.id, expected.version
    );
    let official = is_stats_url(url);
    #[cfg(test)]
    let official = official || (url.scheme() == "http" && url.host_str() == Some("127.0.0.1"));
    if !official || url.query().is_some() || url.path() != path {
        return Err(invalid());
    }
    Ok(())
}

fn bytes(response: Response, max: u64) -> AppResult<Vec<u8>> {
    if response.content_length().is_some_and(|len| len > max) {
        return Err(metadata_error());
    }
    let mut out = Vec::new();
    response
        .take(max + 1)
        .read_to_end(&mut out)
        .map_err(|_| network())?;
    if out.len() as u64 > max {
        return Err(metadata_error());
    }
    Ok(out)
}

fn get(client: &Client, url: &Url, attempt: Option<&str>) -> AppResult<Response> {
    for retry in 0..3 {
        let mut request = client.get(url.clone()).header(ACCEPT_ENCODING, "identity");
        if let Some(attempt) = attempt {
            request = request.header("Idempotency-Key", attempt);
        }
        let result = request.send();
        match result {
            // 对 429，以及带 Retry-After 的故障不立即重试，让用户稍后重试。
            Ok(response)
                if !response.status().is_server_error()
                    || response
                        .headers()
                        .contains_key(reqwest::header::RETRY_AFTER) =>
            {
                return Ok(response);
            }
            _ if retry < 2 => std::thread::sleep(Duration::from_millis(250 * (1 << retry))),
            _ => return Err(network()),
        }
    }
    Err(network())
}

fn check_bytes(bytes: &[u8], expected: &Expected<'_>, official: bool) -> AppResult<()> {
    if bytes.len() as u64 != expected.size {
        return Err(metadata_error());
    }
    use sha2::Digest;
    let actual = resources::to_hex(&sha2::Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected.sha256) {
        return Err(AppError::new("package.checksum_mismatch")
            .with("expected", expected.sha256)
            .with("actual", actual));
    }
    let package = tf_package::open(bytes).map_err(package_error)?;
    check_package(&package, expected, official)
}

fn check_package(
    package: &tf_package::Package,
    expected: &Expected<'_>,
    official: bool,
) -> AppResult<()> {
    if package.manifest.id != expected.id || package.manifest.version != expected.version {
        return Err(metadata_error());
    }
    let signed = matches!(package.trust, tf_package::Trust::Official { .. });
    if official && !signed {
        return Err(AppError::new("market.unsigned").with("id", expected.id));
    }
    // 官方包不能借助第三方下载入口绕过统计；本地导入仍由原有确认流程管理。
    if !official && signed {
        return Err(invalid());
    }
    Ok(())
}

/// 第一跳必须是官方 API；不接受 COS 直链，不自动降级。
pub(crate) fn fetch_official(
    url: &str,
    expected: &Expected<'_>,
    store: &Store,
) -> AppResult<Vec<u8>> {
    let url = Url::parse(url).map_err(|_| invalid())?;
    validate_api(&url, expected)?;
    let client = client(Duration::from_secs(180))?;
    let attempt = uuid::Uuid::new_v4().simple().to_string();
    fetch_attempt(&client, &url, expected, store, &attempt)
}

fn fetch_attempt(
    client: &Client,
    url: &Url,
    expected: &Expected<'_>,
    store: &Store,
    attempt: &str,
) -> AppResult<Vec<u8>> {
    let response = get(client, url, Some(attempt))?;
    if response.status() != reqwest::StatusCode::FOUND {
        return Err(AppError::new("resource.http_status")
            .with("host", "stats.toolforge.devlive.org")
            .with("status", response.status().as_u16()));
    }
    let header = |name: &str| -> AppResult<&str> {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(invalid)
    };
    if header("X-ToolForge-Attempt-Id")? != attempt {
        return Err(metadata_error());
    }
    let receipt = header("X-ToolForge-Receipt")?.to_owned();
    if receipt.is_empty() || receipt.len() > 4096 || !receipt.is_ascii() {
        return Err(invalid());
    }
    let now = now_millis();
    let expires_at = header("X-ToolForge-Receipt-Expires")?
        .parse::<i64>()
        .ok()
        .and_then(|value| value.checked_mul(1000))
        .ok_or_else(invalid)?;
    if expires_at <= now || expires_at > now + MAX_RECEIPT_AGE + 60_000 {
        return Err(invalid());
    }
    let location = Url::parse(header(LOCATION.as_str())?).map_err(|_| invalid())?;
    if !allowed_asset(&location) {
        return Err(invalid());
    }
    // 使用全新请求，不携带 Idempotency-Key、回报凭据或统计站点的鉴权头。
    let asset = get(client, &location, None)?;
    if asset.status() != reqwest::StatusCode::OK {
        return Err(AppError::new("resource.http_status")
            .with("host", "plugins.toolforge.devlive.org")
            .with("status", asset.status().as_u16()));
    }
    let out = bytes(asset, expected.size)?;
    check_bytes(&out, expected, true)?;
    let report = DownloadReport {
        attempt_id: attempt.to_owned(),
        receipt,
        sha256: expected.sha256.to_ascii_lowercase(),
        size_bytes: expected.size,
        expires_at,
        created_at: now_millis(),
        next_attempt_at: now_millis(),
        attempts: 0,
    };
    if let Err(error) = store.download_report_enqueue(&report) {
        log::warn!(
            "could not persist plugin download confirmation: {}",
            error.code
        );
    }
    Ok(out)
}

/// 手动添加的第三方 URL 不产生官方统计，安装仍要求用户确认未信任签名。
pub(crate) fn fetch_third_party(url: &str, expected: &Expected<'_>) -> AppResult<Vec<u8>> {
    validate_expected(expected)?;
    let parsed = Url::parse(url).map_err(|_| invalid())?;
    if !clean_https(&parsed)
        || is_stats_url(&parsed)
        || matches!(
            parsed.host_str(),
            Some(
                "plugins.toolforge.devlive.org"
                    | "toolforge-1258585472.cos.ap-singapore.myqcloud.com"
            )
        )
    {
        return Err(invalid());
    }
    // 第三方来源也不把底层网络错误里的 URL 查询参数透传给前端或日志。
    let out = resources::fetch_bytes(url, MAX_PACKAGE).map_err(|_| network())?;
    check_bytes(&out, expected, false)?;
    Ok(out)
}

#[cfg(test)]
#[path = "plugin_download_test.rs"]
mod tests;
