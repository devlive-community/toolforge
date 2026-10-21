//! Windows：当前用户 WinINet 配置（WinHttpGetIEProxyConfigForCurrentUser）与
//! WinHTTP 默认配置（WinHttpGetDefaultProxyConfiguration）分别展示。

use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_NAMED_PROXY, WINHTTP_ACCESS_TYPE_NO_PROXY,
    WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WINHTTP_PROXY_INFO, WinHttpGetDefaultProxyConfiguration,
    WinHttpGetIEProxyConfigForCurrentUser,
};

use super::super::model::MAX_EXCLUSIONS;
use super::super::model::{Entry, Source, SourceFailure, check_entry};
use crate::mask::mask_url;

/// 失败占位：来源 id + 错误码
fn fail(id: &'static str, code: &str) -> SourceFailure {
    SourceFailure {
        id: id.to_owned(),
        code: code.to_owned(),
    }
}

/// Copies an optional Windows UTF-16 string before its allocation is freed.
///
/// # Safety
/// A non-null pointer must refer to an aligned, readable, null-terminated UTF-16
/// allocation that remains valid and unmodified for this call.
unsafe fn read_pwstr(pointer: *const u16) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    let mut length = 0usize;
    // SAFETY: The caller guarantees a readable, null-terminated allocation,
    // so every element through the terminating zero remains in bounds.
    while unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: The non-null, aligned allocation remains valid and unmodified;
    // the scan above established the readable slice length before its zero.
    let text = unsafe { std::slice::from_raw_parts(pointer, length) };
    Some(String::from_utf16_lossy(text))
}

/// 拆 host:port；IPv6 "[::1]:8080" 去方括号
fn parse_host_port(addr: &str) -> Option<(String, u32)> {
    let addr = addr.trim();
    if let Some(rest) = addr.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = tail.strip_prefix(':')?;
        return Some((host.to_owned(), port.parse().ok()?));
    }
    let (host, port) = addr.rsplit_once(':')?;
    if host.contains(':') {
        // 无方括号的裸 IPv6（无端口），无法安全拆分
        return None;
    }
    Some((host.to_owned(), port.parse().ok()?))
}

/// 解析 "host:port" 或 "http=host:port;https=host:port;..." 形式的代理串；
/// IPv6 地址带方括号，空段跳过
fn parse_win_proxy_list(text: &str) -> Vec<(&'static str, String, u32)> {
    let mut out = Vec::new();
    for part in text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (protocol, addr) = match part.split_once('=') {
            // "http=host:port"：协议显式给出（windows 不再细分 https 键，https 走 http 值）
            Some((scheme, addr)) => (
                match scheme.trim().to_ascii_lowercase().as_str() {
                    "https" => "https",
                    "socks" => "socks",
                    "ftp" => "ftp",
                    _ => "http",
                },
                addr,
            ),
            // "host:port"：单一代理服务所有协议
            None => ("http", part),
        };
        if let Some((host, port)) = parse_host_port(addr) {
            out.push((protocol, host, port));
        }
    }
    out
}

/// 当前用户 WinINet（IE）配置；返回的结构由 CoTaskMemFree 释放
pub fn wininet_current_user() -> Result<Source, SourceFailure> {
    unsafe {
        let mut config: WINHTTP_CURRENT_USER_IE_PROXY_CONFIG = std::mem::zeroed();
        if WinHttpGetIEProxyConfigForCurrentUser(&mut config) == 0 {
            return Err(fail("win-inet", "proxy.wininet_failed"));
        }
        let auto_detect = config.fAutoDetect != 0;
        let pac_url = read_pwstr(config.lpszAutoConfigUrl).map(|url| mask_url(&url));
        let proxy_text = read_pwstr(config.lpszProxy).unwrap_or_default();
        let bypass = read_pwstr(config.lpszProxyBypass).unwrap_or_default();
        // 结构体字段由 CoTaskMemFree 释放（WinINet 分配）
        windows_sys::Win32::System::Com::CoTaskMemFree(config.lpszAutoConfigUrl.cast());
        windows_sys::Win32::System::Com::CoTaskMemFree(config.lpszProxy.cast());
        windows_sys::Win32::System::Com::CoTaskMemFree(config.lpszProxyBypass.cast());
        finish(
            "win-inet",
            auto_detect,
            pac_url.clone(),
            &proxy_text,
            &bypass,
            ie_raw(auto_detect, &pac_url, &proxy_text, &bypass),
        )
    }
}

/// WinHTTP 默认（机器级）配置；字段由 GlobalFree 释放
pub fn winhttp_default() -> Result<Source, SourceFailure> {
    unsafe {
        let mut info: WINHTTP_PROXY_INFO = std::mem::zeroed();
        if WinHttpGetDefaultProxyConfiguration(&mut info) != 0 {
            let named = info.dwAccessType == WINHTTP_ACCESS_TYPE_NAMED_PROXY;
            let no_proxy = info.dwAccessType == WINHTTP_ACCESS_TYPE_NO_PROXY;
            let proxy_text = read_pwstr(info.lpszProxy).unwrap_or_default();
            let bypass = read_pwstr(info.lpszProxyBypass).unwrap_or_default();
            if !proxy_text.is_empty() {
                GlobalFree(info.lpszProxy.cast());
            }
            if !bypass.is_empty() {
                GlobalFree(info.lpszProxyBypass.cast());
            }
            let _ = no_proxy;
            return finish(
                "win-http",
                false,
                None,
                &proxy_text,
                &bypass,
                winhttp_raw(named, &proxy_text, &bypass),
            );
        }
        Err(fail("win-http", "proxy.winhttp_failed"))
    }
}

/// 从代理串与 bypass 生成来源：命名代理即 Configured，否则 Disabled/NotConfigured
fn finish(
    id: &'static str,
    auto_detect: bool,
    pac_url: Option<String>,
    proxy_text: &str,
    bypass: &str,
    raw: serde_json::Value,
) -> Result<Source, SourceFailure> {
    let entries = parse_win_proxy_list(proxy_text)
        .into_iter()
        .map(|(protocol, host, port)| {
            let warnings = check_entry(&host, port);
            Entry {
                protocol: protocol.to_owned(),
                host,
                port,
                warnings,
            }
        })
        .collect();
    let exclusions = bypass
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .take(MAX_EXCLUSIONS)
        .map(str::to_owned)
        .collect();
    Ok(super::super::model::build_source(
        id,
        "user",
        entries,
        auto_detect.then_some(true),
        pac_url,
        exclusions,
        Some(raw),
    ))
}

fn ie_raw(
    auto_detect: bool,
    pac_url: &Option<String>,
    proxy: &str,
    bypass: &str,
) -> serde_json::Value {
    super::super::mask::mask_raw(&serde_json::json!({
        "source": "WinINet (current user)",
        "autoDetect": auto_detect,
        "autoConfigUrl": pac_url,
        "proxy": proxy,
        "proxyBypass": bypass,
    }))
}

fn winhttp_raw(named: bool, proxy: &str, bypass: &str) -> serde_json::Value {
    super::super::mask::mask_raw(&serde_json::json!({
        "source": "WinHTTP (default)",
        "accessType": if named { "named" } else { "no_proxy" },
        "proxy": proxy,
        "proxyBypass": bypass,
    }))
}

#[cfg(test)]
#[path = "windows_test.rs"]
mod tests;
