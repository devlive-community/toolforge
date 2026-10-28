//! 插件 manifest 检查：minAppVersion 必须是 `x.y.z`；新增或递增过版本的插件必须声明 minAppVersion，
//! 并为每个函数写明 CLI / Agent 接口（声明或 false）。已写的接口声明总是严格校验。
//!
//! 「递增过版本」以最近的 v* 标签为基准；取不到标签（例如浅克隆）时只检查格式。

use std::path::Path;

use serde_json::Value;

use crate::interfaces::{self, Plugin};
use crate::notes::git;
use crate::rules::Violation;

const MANIFEST_ROOTS: &[&str] = &["plugins", "market"];

/// 接口声明规则生效（2026-10-22）前已开发、尚未递增的插件：在这些版本上暂不要求接口声明。
/// 下次递增版本时必须补齐，并从这里删除对应条目
const UNDECLARED_ALLOWED: &[(&str, &str)] = &[
    ("market/electricity-cost", "0.1.0"),
    ("market/http-cache", "0.1.0"),
];

/// 与基准相比插件的状态
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Baseline<'a> {
    /// 没有可比较的基准
    Unknown,
    /// 基准中不存在，属于新增插件
    Missing,
    /// 基准中的 version
    Version(&'a str),
}

pub fn valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

impl Baseline<'_> {
    /// 插件相对基准是新增或递增过版本
    pub fn changed(self, manifest: &Value) -> bool {
        match self {
            Self::Unknown => false,
            Self::Missing => true,
            Self::Version(base) => manifest.get("version").and_then(Value::as_str) != Some(base),
        }
    }
}

/// 检查一份 manifest 的 minAppVersion；baseline 为基准中的状态
pub fn check(relative: &str, manifest: &Value, baseline: Baseline<'_>) -> Option<Violation> {
    let location = |line: usize| format!("{relative}:{line}");
    match manifest.get("minAppVersion") {
        Some(Value::String(min)) if valid_version(min) => None,
        Some(other) => Some(Violation {
            rule: "plugin-min-app-version",
            message: format!("minAppVersion must be a plain x.y.z version, found {other}"),
            location: location(1),
        }),
        None => baseline.changed(manifest).then(|| Violation {
            rule: "plugin-min-app-version",
            message: "new or updated plugins must declare minAppVersion".to_owned(),
            location: location(1),
        }),
    }
}

/// 是否要求每个函数都写明接口声明
pub fn require_interfaces(slug_path: &str, manifest: &Value, baseline: Baseline<'_>) -> bool {
    let version = manifest.get("version").and_then(Value::as_str);
    baseline.changed(manifest)
        && !UNDECLARED_ALLOWED
            .iter()
            .any(|(path, allowed)| *path == slug_path && version == Some(*allowed))
}

/// 读取插件声明的各语言包（locales/<语言>.json）
fn locales(dir: &Path, manifest: &Value) -> Vec<(String, Value)> {
    manifest
        .get("locales")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|locale| {
            let messages = std::fs::read_to_string(dir.join(format!("locales/{locale}.json")))
                .ok()
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or(Value::Null);
            (locale.to_owned(), messages)
        })
        .collect()
}

pub fn scan(root: &Path) -> Vec<Violation> {
    let tag = git(root, &["describe", "--tags", "--abbrev=0", "--match", "v*"]).ok();
    if tag.is_none() {
        println!("  no v* tag found; checking minAppVersion format only");
    }
    let mut violations = Vec::new();
    for base in MANIFEST_ROOTS {
        let Ok(entries) = std::fs::read_dir(root.join(base)) else {
            continue;
        };
        let mut slugs: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().join("manifest.json").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        slugs.sort();
        for slug in slugs {
            let relative = format!("{base}/{slug}/manifest.json");
            let Ok(content) = std::fs::read_to_string(root.join(&relative)) else {
                continue;
            };
            let Ok(manifest) = serde_json::from_str::<Value>(&content) else {
                violations.push(Violation {
                    rule: "plugin-manifest",
                    message: "manifest.json is not valid JSON".to_owned(),
                    location: format!("{relative}:1"),
                });
                continue;
            };
            let previous = tag.as_ref().map(|tag| {
                git(root, &["show", &format!("{tag}:{relative}")])
                    .ok()
                    .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            });
            let base_version = previous
                .as_ref()
                .and_then(|p| p.as_ref())
                .and_then(|p| p.get("version"))
                .and_then(Value::as_str);
            let baseline = match (&previous, base_version) {
                (None, _) => Baseline::Unknown,
                (Some(_), Some(version)) => Baseline::Version(version),
                (Some(_), None) => Baseline::Missing,
            };
            violations.extend(check(&relative, &manifest, baseline));
            let slug_path = format!("{base}/{slug}");
            let locales = locales(&root.join(&slug_path), &manifest);
            let plugin = Plugin {
                relative: &relative,
                manifest: &manifest,
                locales: &locales,
            };
            violations.extend(interfaces::check(
                &plugin,
                require_interfaces(&slug_path, &manifest, baseline),
            ));
        }
    }
    violations
}

#[cfg(test)]
#[path = "manifests_test.rs"]
mod tests;
