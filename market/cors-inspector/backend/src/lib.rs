//! CORS 预检判定（在线安装插件）：离线解释 Fetch CORS 规则，按请求分析、
//! 预检响应、实际响应三个阶段逐条件给出允许/阻止/无法判定与可读响应头清单。
//! 不发送 OPTIONS 或实际请求，不模拟 CSP、混合内容、PNA、SameSite 等浏览器行为。

mod fetch;
mod verdict;

use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use crate::fetch::{HeaderArg, ResponseArg};

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

/// 插件实例状态：最近一次判定结果（导出用）。宿主侧单例长期存在，测试可新建实例隔离。
pub struct CorsInspector {
    manifest: Manifest,
    last: Mutex<Option<(u64, Arc<verdict::Report>)>>,
}

impl Default for CorsInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            last: Mutex::new(None),
        }
    }
}

fn store(
    cache: &Mutex<Option<(u64, Arc<verdict::Report>)>>,
    report: verdict::Report,
) -> (u64, Arc<verdict::Report>) {
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    let handle = cache.as_ref().map_or(1, |(t, _)| t + 1);
    let report = Arc::new(report);
    *cache = Some((handle, report.clone()));
    (handle, report)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    document_url: String,
    target_url: String,
    method: String,
    #[serde(default)]
    credentials: Option<String>,
    #[serde(default)]
    headers: Vec<HeaderArg>,
    #[serde(default)]
    preflight: Option<ResponseArg>,
    #[serde(default)]
    actual: Option<ResponseArg>,
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

impl CorsInspector {
    fn inspect(&self, args: InspectArgs) -> PluginResult<Value> {
        let report = verdict::run(
            &args.document_url,
            &args.target_url,
            &args.method,
            args.credentials.as_deref().unwrap_or("same-origin"),
            &args.headers,
            args.preflight.as_ref(),
            args.actual.as_ref(),
        )?;
        let (handle, report) = store(&self.last, report);
        to_value(json!({
            "handle": handle,
            "request": report.request,
            "preflight": report.preflight,
            "reading": report.reading,
            "overall": report.overall,
            "outcome": report.outcome,
            "diagnostics": report.diagnostics,
        }))
    }
}

impl ToolPlugin for CorsInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "inspect" => self.inspect(parse_args(args)?),
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let cache = self.last.lock().unwrap_or_else(|e| e.into_inner());
                let Some((_, report)) = cache.as_ref() else {
                    return Err(PluginError::new("cors.no_result"));
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

tf_plugin_guest::export_plugin!(CorsInspector);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
