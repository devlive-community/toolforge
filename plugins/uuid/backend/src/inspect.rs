use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};
use uuid::{Uuid, Variant};

#[derive(Deserialize)]
pub struct Args {
    input: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// uuid / ulid
    pub kind: &'static str,
    pub canonical: String,
    pub version: Option<usize>,
    pub variant: Option<&'static str>,
    /// UTC 时间（RFC 3339），仅 v1 / v6 / v7 / ULID 有
    pub timestamp: Option<String>,
    pub unix_ms: Option<i64>,
    pub hex: String,
    pub is_nil: bool,
    pub is_max: bool,
}

fn variant_name(variant: Variant) -> &'static str {
    match variant {
        Variant::NCS => "ncs",
        Variant::RFC4122 => "rfc4122",
        Variant::Microsoft => "microsoft",
        _ => "future",
    }
}

fn format_ms(unix_ms: i64) -> Option<String> {
    jiff::Timestamp::from_millisecond(unix_ms)
        .ok()
        .map(|t| t.to_string())
}

pub fn run(args: Args) -> PluginResult<Info> {
    let input = args
        .input
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}');
    if input.is_empty() {
        return Err(PluginError::new("uuid.empty"));
    }

    if let Ok(id) = Uuid::parse_str(input) {
        let unix_ms = id.get_timestamp().map(|ts| {
            let (secs, nanos) = ts.to_unix();
            secs as i64 * 1000 + (nanos / 1_000_000) as i64
        });
        return Ok(Info {
            kind: "uuid",
            canonical: id.hyphenated().to_string(),
            version: Some(id.get_version_num()),
            variant: Some(variant_name(id.get_variant())),
            timestamp: unix_ms.and_then(format_ms),
            unix_ms,
            hex: id.simple().to_string(),
            is_nil: id.is_nil(),
            is_max: id.is_max(),
        });
    }

    if input.len() == 26
        && let Ok(id) = ulid::Ulid::from_string(input)
    {
        let unix_ms = id.timestamp_ms() as i64;
        return Ok(Info {
            kind: "ulid",
            canonical: id.to_string(),
            version: None,
            variant: None,
            timestamp: format_ms(unix_ms),
            unix_ms: Some(unix_ms),
            hex: format!("{:032x}", u128::from(id)),
            is_nil: id.is_nil(),
            is_max: u128::from(id) == u128::MAX,
        });
    }

    Err(PluginError::new("uuid.invalid"))
}

#[cfg(test)]
#[path = "inspect_test.rs"]
mod tests;
