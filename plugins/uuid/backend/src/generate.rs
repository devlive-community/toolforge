use std::time::Instant;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};
use uuid::Uuid;

pub const MAX_COUNT: usize = 1000;
const DEFAULT_NANOID_SIZE: usize = 21;
const MAX_NANOID_SIZE: usize = 256;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    V1,
    V3,
    V4,
    V5,
    V7,
    Ulid,
    Nanoid,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Namespace {
    #[default]
    Dns,
    Url,
    Oid,
    X500,
    Custom,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub kind: Kind,
    #[serde(default = "one")]
    pub count: usize,
    #[serde(default)]
    pub uppercase: bool,
    #[serde(default = "yes")]
    pub hyphens: bool,
    #[serde(default)]
    pub braces: bool,
    #[serde(default)]
    pub namespace: Namespace,
    #[serde(default)]
    pub custom_namespace: String,
    #[serde(default)]
    pub name: String,
    pub nanoid_size: Option<usize>,
    pub nanoid_alphabet: Option<String>,
}

fn one() -> usize {
    1
}

fn yes() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub ids: Vec<String>,
    /// v3 / v5 为确定性结果，数量固定为 1
    pub deterministic: bool,
    pub elapsed_ms: f64,
}

fn namespace(args: &Args) -> PluginResult<Uuid> {
    Ok(match args.namespace {
        Namespace::Dns => Uuid::NAMESPACE_DNS,
        Namespace::Url => Uuid::NAMESPACE_URL,
        Namespace::Oid => Uuid::NAMESPACE_OID,
        Namespace::X500 => Uuid::NAMESPACE_X500,
        Namespace::Custom => Uuid::parse_str(args.custom_namespace.trim())
            .map_err(|_| PluginError::new("uuid.invalid_namespace"))?,
    })
}

fn format_uuid(id: Uuid, args: &Args) -> String {
    let mut text = if args.hyphens {
        id.hyphenated().to_string()
    } else {
        id.simple().to_string()
    };
    if args.uppercase {
        text.make_ascii_uppercase();
    }
    if args.braces {
        text = format!("{{{text}}}");
    }
    text
}

fn nanoid(args: &Args) -> PluginResult<String> {
    let size = args.nanoid_size.unwrap_or(DEFAULT_NANOID_SIZE);
    if size == 0 || size > MAX_NANOID_SIZE {
        return Err(PluginError::new("uuid.invalid_size").with("max", MAX_NANOID_SIZE));
    }
    match args.nanoid_alphabet.as_deref().filter(|a| !a.is_empty()) {
        Some(alphabet) => {
            let chars: Vec<char> = alphabet.chars().collect();
            if chars.len() < 2 {
                return Err(PluginError::new("uuid.invalid_alphabet"));
            }
            Ok(nanoid::nanoid!(size, &chars))
        }
        None => Ok(nanoid::nanoid!(size)),
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.count == 0 || args.count > MAX_COUNT {
        return Err(PluginError::new("uuid.invalid_count").with("max", MAX_COUNT));
    }
    let start = Instant::now();
    let deterministic = matches!(args.kind, Kind::V3 | Kind::V5);
    let count = if deterministic { 1 } else { args.count };

    let ids = (0..count)
        .map(|_| -> PluginResult<String> {
            Ok(match args.kind {
                Kind::V1 => {
                    // 节点 ID 取随机字节（不暴露本机 MAC 地址）
                    let random = Uuid::new_v4();
                    let node: [u8; 6] = random.as_bytes()[..6].try_into().expect("6 bytes");
                    format_uuid(Uuid::now_v1(&node), &args)
                }
                Kind::V3 => format_uuid(
                    Uuid::new_v3(&namespace(&args)?, args.name.as_bytes()),
                    &args,
                ),
                Kind::V4 => format_uuid(Uuid::new_v4(), &args),
                Kind::V5 => format_uuid(
                    Uuid::new_v5(&namespace(&args)?, args.name.as_bytes()),
                    &args,
                ),
                Kind::V7 => format_uuid(Uuid::now_v7(), &args),
                Kind::Ulid => {
                    let id = ulid::Ulid::generate().to_string();
                    if args.uppercase {
                        id
                    } else {
                        id.to_ascii_lowercase()
                    }
                }
                Kind::Nanoid => nanoid(&args)?,
            })
        })
        .collect::<PluginResult<Vec<_>>>()?;

    Ok(Output {
        ids,
        deterministic,
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

#[cfg(test)]
#[path = "generate_test.rs"]
mod tests;
