//! 重定向映射后端：旧→新 URL 对生成 Nginx/Apache/Cloudflare 301 规则，
//! 链式展平（A→B→C 输出 A→C）与环路检测。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct RedirectMap {
    manifest: Manifest,
}

impl Default for RedirectMap {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for RedirectMap {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(RedirectMap);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// 旧 → 新 映射（相对路径或完整 URL）
    pub mappings: Vec<(String, String)>,
    /// nginx / apache / cloudflare
    pub format: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Redirect {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub format: String,
    pub rules: String,
    pub redirects: Vec<Redirect>,
    /// 环路（映射链成环，未生成规则）
    pub cycles: Vec<String>,
    /// 多对一冲突（同一 from 出现多次，取首个）
    pub conflicts: Vec<String>,
    pub chains_flattened: usize,
}

pub fn generate(args: Args) -> PluginResult<Output> {
    let format = args.format.to_lowercase();
    if !matches!(format.as_str(), "nginx" | "apache" | "cloudflare") {
        return Err(PluginError::new("redirect.bad_format").with("format", args.format));
    }
    if args.mappings.is_empty() {
        return Err(PluginError::new("redirect.no_mappings"));
    }
    if args.mappings.len() > 5000 {
        return Err(PluginError::new("redirect.too_many").with("max", 5000));
    }
    // 归一化（去尾斜杠不一致）
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut conflicts = Vec::new();
    for (from, to) in &args.mappings {
        let from = from.trim().trim_end_matches('/').to_owned();
        let to = to.trim().trim_end_matches('/').to_owned();
        if from.is_empty() || to.is_empty() {
            return Err(PluginError::new("redirect.empty_mapping"));
        }
        if map.contains_key(&from) {
            conflicts.push(from.clone());
            continue;
        }
        map.insert(from, to);
    }
    // 链式展平 + 环路检测
    let mut chains_flattened = 0usize;
    let mut cycles = Vec::new();
    let mut resolved: Vec<Redirect> = Vec::new();
    for from in map.keys() {
        let mut current = from.clone();
        let mut steps = 0usize;
        let mut chain_visited = std::collections::HashSet::new();
        chain_visited.insert(current.clone());
        let mut cycle = false;
        while let Some(next) = map.get(&current) {
            if chain_visited.contains(next) {
                // 环：同一条环只报告一次（用链上最小 key 作为标识）
                let mut members: Vec<String> = chain_visited.iter().cloned().collect();
                members.sort();
                let cycle_text = format!("cycle:{}", members.join("→"));
                if !cycles.contains(&cycle_text) {
                    cycles.push(cycle_text);
                }
                cycle = true;
                break;
            }
            if steps >= 32 {
                cycle = true;
                break;
            }
            chain_visited.insert(next.clone());
            current = next.clone();
            steps += 1;
        }
        if cycle {
            continue;
        }
        // current 已是链终点（不是任何映射的 from）
        if &current != from {
            chains_flattened += 1;
        }
        let final_target = current.clone();
        resolved.push(Redirect {
            from: from.clone(),
            to: final_target,
        });
    }
    resolved.sort_by(|a, b| a.from.cmp(&b.from));
    // 生成规则文本
    let mut rules = String::new();
    match format.as_str() {
        "nginx" => {
            for redirect in &resolved {
                rules.push_str(&format!(
                    "location = {} {{ return 301 {}; }}\n",
                    redirect.from, redirect.to
                ));
            }
        }
        "apache" => {
            rules.push_str("<IfModule mod_rewrite.c>\nRewriteEngine On\n");
            for redirect in &resolved {
                rules.push_str(&format!(
                    "RewriteRule ^{}$ {} [R=301,L]\n",
                    redirect.from.trim_start_matches('/'),
                    redirect.to
                ));
            }
            rules.push_str("</IfModule>\n");
        }
        _ => {
            // Cloudflare Bulk Redirects CSV 格式
            for redirect in &resolved {
                rules.push_str(&format!("{},{}\n", redirect.from, redirect.to));
            }
        }
    }
    Ok(Output {
        format: format.clone(),
        rules,
        redirects: resolved,
        cycles,
        conflicts,
        chains_flattened,
    })
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
