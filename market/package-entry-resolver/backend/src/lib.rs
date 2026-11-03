//! Node 包入口解析器后端：按 Node.js「Package Exports 解析」规范离线解释
//! package.json 的 `exports` 字段，逐步展示条件命中、数组回退、pattern 展开与
//! null 禁止。对象键序由 serde_json 的 preserve_order 保留（规范要求不排序）。
//!
//! 首版仅覆盖单包 exports 的目标选择：不做 node_modules 查找、main 回退或磁盘检查；
//! 可附带包内相对路径清单做「清单核对」（不冒充实际读取）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 输入规模上限（超限给具体原因，防止恶意大输入）
const MAX_JSON_BYTES: usize = 128 * 1024;
const MAX_DEPTH: usize = 10;
const MAX_KEYS: usize = 128;

pub struct PackageEntryResolver {
    manifest: Manifest,
}

impl Default for PackageEntryResolver {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for PackageEntryResolver {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "resolve" => to_value(resolve(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(PackageEntryResolver);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// package.json 的完整内容（JSON 文本，保留原始键序）
    pub package_json: String,
    /// 请求的包内子路径（"." 或 "./lib/util" 等）
    pub subpath: String,
    /// 启用的条件集合（如 ["import", "node"]；"default" 恒可用）
    #[serde(default)]
    pub conditions: Vec<String>,
    /// 可选：包内相对路径清单（如 dist/index.js），用于核对最终目标
    #[serde(default)]
    pub listing: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// step.<kind>：condition / array_item / target / blocked
    pub kind: &'static str,
    /// 命中或跳过的条件名 / 数组序号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// 结果说明键（outcomes.<key>）
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub subpath: String,
    pub conditions: Vec<String>,
    /// resolved / blocked / no_match / invalid_target / invalid_config / not_in_listing
    pub status: &'static str,
    /// 最终解析目标（相对包根，如 ./dist/index.mjs）
    pub target: Option<String>,
    /// 清单核对结果（提供清单且解析成功时）
    pub listed: Option<bool>,
    pub steps: Vec<Step>,
    pub node_version: &'static str,
}

/// 条件匹配：default 恒可用；未启用条件的分支要逐步展示为 skipped
fn match_conditions(
    object: &serde_json::Map<String, Value>,
    conditions: &[String],
    pattern: Option<&str>,
    steps: &mut Vec<Step>,
    depth: usize,
) -> Result<Option<String>, &'static str> {
    if depth > MAX_DEPTH {
        return Err("invalid_config");
    }
    if object.len() > MAX_KEYS {
        return Err("too_many_keys");
    }
    for (key, target) in object {
        if key == "default" || conditions.iter().any(|c| c == key) {
            steps.push(Step {
                kind: "condition",
                key: Some(key.clone()),
                outcome: "condition_hit",
                target: None,
            });
            match resolve_target(target, pattern, conditions, steps, depth + 1)? {
                Some(resolved) => return Ok(Some(resolved)),
                // no_match：继续尝试后面的条件分支
                None => continue,
            }
        } else {
            steps.push(Step {
                kind: "condition",
                key: Some(key.clone()),
                outcome: "condition_skipped",
                target: None,
            });
        }
    }
    Ok(None)
}

/// NODE 的 PACKAGE_TARGET_RESOLVE：字符串 / null / 数组 / 对象
fn resolve_target(
    target: &Value,
    pattern: Option<&str>,
    conditions: &[String],
    steps: &mut Vec<Step>,
    depth: usize,
) -> Result<Option<String>, &'static str> {
    if depth > MAX_DEPTH {
        return Err("invalid_config");
    }
    match target {
        Value::String(text) => {
            let status = if text.starts_with("./") {
                "target_ok"
            } else {
                "target_invalid"
            };
            steps.push(Step {
                kind: "target",
                key: None,
                outcome: status,
                target: Some(text.clone()),
            });
            if status == "target_ok" {
                let resolved = match pattern {
                    Some(matched) => text.replacen('*', matched, 1),
                    None => text.clone(),
                };
                if resolved.contains("../") || resolved.contains("..\\") {
                    return Err("invalid_target");
                }
                Ok(Some(resolved))
            } else {
                // 字符串目标非法：按规范不算「无匹配」，直接判配置非法
                Err("invalid_target")
            }
        }
        Value::Null => {
            steps.push(Step {
                kind: "blocked",
                key: None,
                outcome: "blocked",
                target: None,
            });
            Err("blocked")
        }
        Value::Array(items) => {
            if items.len() > MAX_KEYS {
                return Err("too_many_keys");
            }
            let mut saw_invalid = false;
            for item in items {
                match resolve_target(item, pattern, conditions, steps, depth + 1) {
                    Ok(Some(resolved)) => return Ok(Some(resolved)),
                    Ok(None) => continue,
                    Err("invalid_target") => {
                        saw_invalid = true;
                        continue;
                    }
                    Err(other) => return Err(other),
                }
            }
            // 数组里出现非法目标且全部不可用：按规范报 invalid_target 而不是 no_match
            if saw_invalid {
                Err("invalid_target")
            } else {
                Ok(None)
            }
        }
        Value::Object(map) => {
            if map.len() > MAX_KEYS {
                return Err("too_many_keys");
            }
            match_conditions(map, conditions, pattern, steps, depth)
        }
        _ => Err("invalid_config"),
    }
}

pub fn resolve(args: Args) -> PluginResult<Output> {
    if args.package_json.len() > MAX_JSON_BYTES {
        return Err(PluginError::new("resolver.too_large").with("max", MAX_JSON_BYTES as u64));
    }
    let package: Value = serde_json::from_str(&args.package_json)
        .map_err(|e| PluginError::new("resolver.bad_json").with("detail", e.to_string()))?;
    let Some(exports) = package.get("exports") else {
        return Err(PluginError::new("resolver.no_exports"));
    };
    let subpath = args.subpath.trim().to_owned();
    if subpath != "." && !subpath.starts_with("./") {
        return Err(PluginError::new("resolver.bad_subpath").with("subpath", subpath));
    }
    let mut steps: Vec<Step> = Vec::new();

    let outcome: Result<Option<String>, &'static str> = match exports {
        // 字符串简写：仅 "." 可用
        Value::String(target) => {
            if subpath == "." {
                resolve_target(
                    &Value::String(target.clone()),
                    None,
                    &args.conditions,
                    &mut steps,
                    0,
                )
            } else {
                steps.push(Step {
                    kind: "target",
                    key: Some(subpath.clone()),
                    outcome: "no_export_key",
                    target: None,
                });
                Ok(None)
            }
        }
        Value::Object(map) => {
            if map.len() > MAX_KEYS {
                return Err(PluginError::new("resolver.too_many_keys").with("max", MAX_KEYS as u64));
            }
            let dotted = map.keys().any(|k| k.starts_with('.'));
            if !dotted {
                // 顶层条件对象（键为条件名），仅 "." 请求可用
                if subpath == "." {
                    resolve_target(exports, None, &args.conditions, &mut steps, 0)
                } else {
                    steps.push(Step {
                        kind: "target",
                        key: Some(subpath.clone()),
                        outcome: "no_export_key",
                        target: None,
                    });
                    Ok(None)
                }
            } else if let Some(target) = map.get(&subpath) {
                // 精确子路径优先
                resolve_target(target, None, &args.conditions, &mut steps, 0)
            } else {
                // pattern 匹配：取 * 前缀最长者
                let mut best: Option<(String, String)> = None;
                for key in map.keys() {
                    let Some((prefix, suffix)) = key.split_once('*') else {
                        continue;
                    };
                    if subpath.starts_with(prefix)
                        && subpath.ends_with(suffix)
                        && subpath.len() >= prefix.len() + suffix.len()
                    {
                        let matched =
                            subpath[prefix.len()..subpath.len() - suffix.len()].to_string();
                        if matched.contains("../") {
                            continue;
                        }
                        let better = best.as_ref().is_none_or(|(existing_key, _)| {
                            let existing_prefix = existing_key
                                .split_once('*')
                                .map(|(p, _)| p.len())
                                .unwrap_or(0);
                            prefix.len() > existing_prefix
                        });
                        if better {
                            best = Some((key.clone(), matched));
                        }
                    }
                }
                match best {
                    Some((key, matched)) => {
                        let target = &map[&key];
                        resolve_target(target, Some(&matched), &args.conditions, &mut steps, 0)
                    }
                    None => {
                        steps.push(Step {
                            kind: "target",
                            key: Some(subpath.clone()),
                            outcome: "no_export_key",
                            target: None,
                        });
                        Ok(None)
                    }
                }
            }
        }
        _ => Err("invalid_config"),
    };

    let (status, target) = match outcome {
        Ok(Some(resolved)) => ("resolved", Some(resolved)),
        Ok(None) => ("no_match", None),
        Err("blocked") => ("blocked", None),
        Err("invalid_target") => ("invalid_target", None),
        Err("invalid_config") => ("invalid_config", None),
        Err("too_many_keys") => {
            return Err(PluginError::new("resolver.too_many_keys").with("max", MAX_KEYS as u64));
        }
        Err(_) => ("no_match", None),
    };
    // 清单核对：只对解析成功的目标做，且明确是清单比对而非磁盘检查
    let listed = match (&target, args.listing.is_empty()) {
        (Some(target), false) => Some(
            args.listing
                .iter()
                .any(|p| p.trim().trim_start_matches("./") == target.trim_start_matches("./")),
        ),
        _ => None,
    };
    let status = match (status, listed) {
        ("resolved", Some(false)) => "not_in_listing",
        (other, _) => other,
    };
    Ok(Output {
        subpath,
        conditions: args.conditions,
        status,
        target,
        listed,
        steps,
        node_version: "Node 16+ (package exports resolution)",
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
