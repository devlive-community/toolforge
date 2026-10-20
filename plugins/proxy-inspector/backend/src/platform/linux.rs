//! Linux：GNOME 代理设置（gsettings 只读）。其他桌面明确未覆盖，不声称
//! Linux 有统一系统代理；gsettings 不存在时报告“来源不支持”。

use std::process::Command;

use super::super::model::{Entry, Source, SourceStatus, check_entry};
use crate::mask::mask_url;

/// 读一个 gsettings 键；gsettings 不可用 → Unsupported（带原因）
fn get(schema: &str, key: &str) -> Result<String, GnomeError> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .map_err(|_| GnomeError::Unsupported)?;
    if !output.status.success() {
        // 键不存在或 schema 未安装：该来源不可用
        return Err(GnomeError::Unsupported);
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok(text)
}

enum GnomeError {
    Unsupported,
}

/// gsettings 值 → JSON 字面量：剥引号、解析 "@as []" 与字符串数组
fn parse_value(text: &str) -> serde_json::Value {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("@as ") {
        return parse_value(rest);
    }
    if text.starts_with('[') {
        let inner = text.trim_start_matches('[').trim_end_matches(']').trim();
        if inner.is_empty() {
            return serde_json::Value::Array(vec![]);
        }
        return serde_json::Value::Array(
            inner
                .split(',')
                .map(|item| parse_value(item.trim()))
                .collect(),
        );
    }
    if let Some(stripped) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return serde_json::Value::String(stripped.to_owned());
    }
    if let Ok(number) = text.parse::<u32>() {
        return serde_json::Value::Number(number.into());
    }
    serde_json::Value::String(text.to_owned())
}

fn host_of(schema: &str) -> Result<String, GnomeError> {
    get(schema, "host")
}

fn port_of(schema: &str) -> Result<u32, GnomeError> {
    get(schema, "port")?
        .parse::<u32>()
        .map_err(|_| GnomeError::Unsupported)
}

fn entry(schema: &str, protocol: &'static str) -> Option<Entry> {
    let host = host_of(schema).ok()?;
    let port = port_of(schema).ok()?;
    Some(Entry {
        protocol: protocol.to_owned(),
        host: crate::mask::mask_host(&host),
        port,
        warnings: check_entry(&host, port),
    })
}

/// GNOME 代理：mode none=未启用、manual=手动、auto=PAC
pub fn gnome_settings() -> Result<Source, crate::model::SourceFailure> {
    let mode = match get("org.gnome.system.proxy", "mode") {
        Ok(mode) => parse_value(&mode).as_str().unwrap_or("none").to_owned(),
        Err(_) => {
            return Ok(Source {
                id: "gnome".to_owned(),
                scope: "system".to_owned(),
                status: SourceStatus::Unsupported,
                entries: vec![],
                auto_discover: None,
                pac_url: None,
                exclusions: vec![],
                raw: None,
            });
        }
    };
    let mut entries = Vec::new();
    if mode == "manual" {
        for (schema, protocol) in [
            ("org.gnome.system.proxy.http", "http"),
            ("org.gnome.system.proxy.https", "https"),
            ("org.gnome.system.proxy.socks", "socks"),
        ] {
            if let Some(entry) = entry(schema, protocol) {
                entries.push(entry);
            }
        }
    }
    let pac_url = (mode == "auto")
        .then(|| get("org.gnome.system.proxy", "autoconfig-url").ok())
        .flatten()
        .map(|url| mask_url(&parse_value(&url).as_str().unwrap_or_default()));
    let exclusions = match get("org.gnome.system.proxy", "ignore-hosts") {
        Ok(text) => parse_value(&text)
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .take(super::super::model::MAX_EXCLUSIONS)
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => vec![],
    };
    let raw = serde_json::json!({
        "source": "GNOME (gsettings org.gnome.system.proxy)",
        "mode": mode,
        "autoconfig-url": pac_url,
        "ignore-hosts": exclusions,
    });
    let raw = super::super::mask::mask_raw(&raw);
    Ok(super::super::model::build_source(
        "gnome",
        "system",
        entries,
        None,
        pac_url,
        exclusions,
        Some(raw),
    ))
}
