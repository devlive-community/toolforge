use jiff::tz::TimeZone;
use serde::Serialize;
use tf_plugin_api::{PluginError, PluginResult};

/// 界面使用的特殊时区名：本机时区
pub const LOCAL: &str = "local";

pub fn resolve(name: &str) -> PluginResult<TimeZone> {
    match name {
        "" | LOCAL => Ok(TimeZone::system()),
        "UTC" => Ok(TimeZone::UTC),
        other => TimeZone::get(other)
            .map_err(|_| PluginError::new("time.invalid_timezone").with("timezone", other)),
    }
}

/// 时区的展示名：本机时区尽量给出 IANA 名称
pub fn display_name(name: &str, tz: &TimeZone) -> String {
    match name {
        "" | LOCAL => tz.iana_name().unwrap_or("Local").to_owned(),
        other => other.to_owned(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Zones {
    pub local: String,
    pub all: Vec<String>,
}

pub fn list() -> Zones {
    let mut all: Vec<String> = jiff::tz::db()
        .available()
        .map(|name| name.as_str().to_owned())
        .collect();
    all.sort();
    all.dedup();
    Zones {
        local: TimeZone::system().iana_name().unwrap_or("UTC").to_owned(),
        all,
    }
}

#[cfg(test)]
#[path = "zone_test.rs"]
mod tests;
