//! CSP 策略检查（在线安装插件）：离线解析 Content-Security-Policy 响应头，
//! 解释指令回退链与来源匹配，并对资源清单逐条给出允许/阻止/无法判定。
//! 不访问资源 URL、不执行脚本、不收集报告。

mod policy;
mod verdict;

use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 诊断：code 由前端 i18n，params 提供插值
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Diag {
    pub code: String,
    #[serde(default)]
    pub params: Value,
}

impl Diag {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            params: Value::Object(serde_json::Map::new()),
        }
    }

    /// 追加一个插值参数
    pub fn param(mut self, key: &str, value: impl Into<Value>) -> Self {
        if let Value::Object(params) = &mut self.params {
            params.insert(key.to_owned(), value.into());
        }
        self
    }
}

/// 最近一次检查结果（导出用）
static LAST: Mutex<Option<(u64, Arc<verdict::Report>)>> = Mutex::new(None);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    policies: String,
    #[serde(default)]
    document_url: Option<String>,
    #[serde(default)]
    resources: Vec<verdict::ResourceInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArgs {
    format: Format,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    Markdown,
    Json,
}

fn store(report: verdict::Report) -> (u64, Arc<verdict::Report>) {
    let mut cache = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let handle = cache.as_ref().map_or(1, |(t, _)| t + 1);
    let report = Arc::new(report);
    *cache = Some((handle, report.clone()));
    (handle, report)
}

fn inspect(args: InspectArgs) -> PluginResult<Value> {
    if args.policies.len() > policy::MAX_TEXT {
        return Err(PluginError::new("csp.too_large").with("kb", policy::MAX_TEXT >> 10));
    }
    if args.resources.len() > policy::MAX_RESOURCES {
        return Err(PluginError::new("csp.too_many_resources").with("max", policy::MAX_RESOURCES));
    }
    let report = verdict::run(
        &args.policies,
        args.document_url.as_deref(),
        &args.resources,
    )?;
    let (handle, report) = store(report);
    to_value(json!({
        "handle": handle,
        "policies": report.policies,
        "resources": report.resources,
        "diagnostics": report.diagnostics,
        "stats": report.stats,
    }))
}

pub struct CspInspector {
    manifest: Manifest,
}

impl Default for CspInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for CspInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => inspect(parse_args(args)?),
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let cache = LAST.lock().unwrap_or_else(|e| e.into_inner());
                let Some((_, report)) = cache.as_ref() else {
                    return Err(PluginError::new("csp.no_result"));
                };
                match args.format {
                    Format::Markdown => to_value(json!({
                        "content": verdict::markdown(report),
                        "ext": "md",
                    })),
                    Format::Json => to_value(json!({
                        "content": serde_json::to_string_pretty(&**report).unwrap_or_default(),
                        "ext": "json",
                    })),
                }
            }
            other => Err(unknown_function(other)),
        }
    }
}

tf_plugin_guest::export_plugin!(CspInspector);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
