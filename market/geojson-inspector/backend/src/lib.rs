//! GeoJSON 结构与坐标检查（在线安装插件）：面向 RFC 7946 的离线诊断。
//! 语法由 serde_json 校验，GeoJSON 语义（成员、嵌套、环闭合、bbox、跨反经线）
//! 由 Rust 侧规则判定，前端只负责定位与展示。不访问网络、不自动修复。

mod rules;
mod spans;

use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

/// 本地文件读取上限
const FILE_MAX: usize = 8 * 1024 * 1024;

/// 报告附加句柄后返回给前端
fn with_handle(handle: u64, report: &rules::Report) -> PluginResult<Value> {
    let mut out = serde_json::to_value(report)
        .map_err(|e| PluginError::new("plugin.serialize").with("detail", e.to_string()))?;
    out["handle"] = Value::from(handle);
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadFileArgs {
    path: String,
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

/// 最近一次判定结果（导出用）。宿主侧单例长期存在，测试可新建实例隔离。
pub struct GeoJsonInspector {
    manifest: Manifest,
    last: Mutex<Option<(u64, Arc<rules::Report>)>>,
}

impl Default for GeoJsonInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            last: Mutex::new(None),
        }
    }
}

fn store(
    cache: &Mutex<Option<(u64, Arc<rules::Report>)>>,
    report: rules::Report,
) -> (u64, Arc<rules::Report>) {
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    let handle = cache.as_ref().map_or(1, |(t, _)| t + 1);
    let report = Arc::new(report);
    *cache = Some((handle, report.clone()));
    (handle, report)
}

/// 读本地 .geojson/.json 文本：仅用户主动选择的路径，带大小与编码守卫
fn read_file(args: ReadFileArgs) -> PluginResult<String> {
    let file = File::open(&args.path).map_err(|_| PluginError::new("geojson.file_read"))?;
    let metadata = file
        .metadata()
        .map_err(|_| PluginError::new("geojson.file_read"))?;
    if !metadata.is_file() {
        return Err(PluginError::new("geojson.not_file"));
    }
    if metadata.len() > FILE_MAX as u64 {
        return Err(PluginError::new("geojson.file_too_large").with("mb", FILE_MAX >> 20));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(FILE_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PluginError::new("geojson.file_read"))?;
    if bytes.len() > FILE_MAX {
        return Err(PluginError::new("geojson.file_too_large").with("mb", FILE_MAX >> 20));
    }
    let text = String::from_utf8(bytes).map_err(|_| PluginError::new("geojson.file_encoding"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

impl GeoJsonInspector {
    fn inspect(&self, args: InspectArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        if args.text.len() > spans::MAX_INPUT {
            return Err(PluginError::new("geojson.too_large").with("mb", spans::MAX_INPUT >> 20));
        }
        ctx.stage("parse");
        let value: Value = match serde_json::from_str(&args.text) {
            Ok(value) => value,
            // 语法错误也给出可读报告（serde_json 自带行列）
            Err(error) => {
                let diag = rules::Diag {
                    level: rules::ERROR,
                    code: "geojson.syntax".to_owned(),
                    params: serde_json::Map::from_iter([(
                        "detail".to_owned(),
                        Value::from(error.to_string()),
                    )]),
                    pointer: String::new(),
                    line: Some(error.line() as u64),
                    column: Some(error.column() as u64),
                };
                let report = rules::Report {
                    overall: "invalid",
                    incomplete: false,
                    stats: {
                        let mut stats = rules::Stats::default();
                        stats.errors = 1;
                        stats
                    },
                    diagnostics: vec![diag],
                };
                let (handle, report) = store(&self.last, report);
                return with_handle(handle, &report);
            }
        };
        ctx.stage("scan");
        let spans = match spans::scan(&args.text) {
            Ok(spans) => spans,
            Err(limit) if limit.kind == "size" => {
                return Err(
                    PluginError::new("geojson.too_large").with("mb", spans::MAX_INPUT >> 20)
                );
            }
            // 深度超限：继续检查但标记未完成，位置可能缺失
            Err(_) => Default::default(),
        };
        ctx.stage("check");
        let report = rules::run(&value, &spans);
        if ctx.is_cancelled() {
            return Err(PluginError::new("task.cancelled"));
        }
        ctx.stage("done");
        let (handle, report) = store(&self.last, report);
        with_handle(handle, &report)
    }
}

impl ToolPlugin for GeoJsonInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "read_file" => to_value(read_file(parse_args(args)?)),
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let cache = self.last.lock().unwrap_or_else(|e| e.into_inner());
                let Some((_, report)) = cache.as_ref() else {
                    return Err(PluginError::new("geojson.no_result"));
                };
                match args.format {
                    Format::Markdown => to_value(json!({
                        "content": markdown(report),
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

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "inspect" => self.inspect(parse_args(args)?, ctx),
            other => Err(unknown_function(other)),
        }
    }
}

/// Markdown 报告：结论 + 统计 + 分级诊断清单
pub fn markdown(report: &rules::Report) -> String {
    let mut out = String::new();
    out.push_str("# GeoJSON Inspection\n\n");
    out.push_str(&format!("- Overall: **{}**\n", report.overall));
    if report.incomplete {
        out.push_str("- Note: the input hit a limit, results are incomplete\n");
    }
    out.push_str(&format!(
        "- Features: {}, positions: {}\n",
        report.stats.features, report.stats.positions
    ));
    if !report.stats.types.is_empty() {
        let types = report
            .stats
            .types
            .iter()
            .map(|(kind, count)| format!("{kind} × {count}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- Geometry types: {types}\n"));
    }
    out.push_str(&format!(
        "- Diagnostics: {} errors, {} warnings, {} hints\n\n",
        report.stats.errors, report.stats.warnings, report.stats.hints
    ));
    for diag in &report.diagnostics {
        out.push_str(&format!(
            "- **{}** `{}` at `{}`{}\n",
            diag.level,
            diag.code,
            if diag.pointer.is_empty() {
                "/"
            } else {
                &diag.pointer
            },
            diag.line
                .map(|line| format!(" (line {line}, column {})", diag.column.unwrap_or(0)))
                .unwrap_or_default(),
        ));
    }
    out
}

tf_plugin_guest::export_plugin!(GeoJsonInspector);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
