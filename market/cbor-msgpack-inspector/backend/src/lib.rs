//! CBOR / MessagePack 检查（在线安装插件）：用户显式选择格式，离线解析单个顶层值，
//! 输出带字节区间与偏移诊断的类型树。不猜测格式、不吞尾随数据、不把多值流当数组。

mod cbor;
mod model;
mod msgpack;

use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

use model::{MAX_BYTES, Report, Stats};

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    /// cbor | msgpack（显式选择，不猜测）
    format: String,
    /// raw 文件路径（fs:user-selected）或 hex/base64 文本，二选一
    #[serde(default)]
    path: Option<String>,
    /// hex | base64（path 缺省时必填）
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default)]
    text: Option<String>,
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

/// 插件实例状态：最近一次判定结果。宿主侧单例长期存在，测试可新建实例隔离。
pub struct BinaryInspector {
    manifest: Manifest,
    last: Mutex<Option<(u64, Arc<Report>)>>,
}

impl Default for BinaryInspector {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            last: Mutex::new(None),
        }
    }
}

fn store(cache: &Mutex<Option<(u64, Arc<Report>)>>, report: Report) -> (u64, Arc<Report>) {
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    let handle = cache.as_ref().map_or(1, |(t, _)| t + 1);
    let report = Arc::new(report);
    *cache = Some((handle, report.clone()));
    (handle, report)
}

fn with_handle(handle: u64, report: &Report) -> PluginResult<Value> {
    let mut out = serde_json::to_value(report)
        .map_err(|e| PluginError::new("plugin.serialize").with("detail", e.to_string()))?;
    out["handle"] = Value::from(handle);
    Ok(out)
}

fn diag_report(
    format: &'static str,
    code: &str,
    params: serde_json::Map<String, Value>,
    offset: Option<u64>,
) -> Report {
    let mut report = Report {
        format,
        overall: "issues",
        incomplete: false,
        stats: Stats::default(),
        root: None,
        trailing: None,
        diagnostics: Vec::new(),
    };
    model::push_report_diag(&mut report, model::ERROR, code, params, offset);
    report
}

fn decode_transport(format: &'static str, encoding: &str, text: &str) -> Result<Vec<u8>, Report> {
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    match encoding {
        "hex" => {
            let cleaned = cleaned.strip_prefix("0x").unwrap_or(&cleaned);
            if cleaned.len() % 2 != 0 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(diag_report(
                    format,
                    "transport.hex",
                    serde_json::Map::new(),
                    None,
                ));
            }
            let bytes = (0..cleaned.len() / 2)
                .map(|index| u8::from_str_radix(&cleaned[index * 2..index * 2 + 2], 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| diag_report(format, "transport.hex", serde_json::Map::new(), None))?;
            Ok(bytes)
        }
        "base64" => base64_decode(&cleaned)
            .ok_or_else(|| diag_report(format, "transport.base64", serde_json::Map::new(), None)),
        _ => Err(diag_report(
            format,
            "transport.encoding",
            serde_json::Map::from_iter([("encoding".to_owned(), Value::from(encoding))]),
            None,
        )),
    }
}

/// base64 标准/URL 安全字母表，忽略填充差异
fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes() {
        let value = base64_value(byte)?;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    // 剩余位数只能是 0/2/4（对应 1/2/3 字节组），越界说明输入非法
    if bits >= 6 {
        return None;
    }
    Some(out)
}

impl BinaryInspector {
    fn inspect(&self, args: InspectArgs, ctx: &dyn TaskContext) -> PluginResult<Value> {
        let format: &'static str = match args.format.as_str() {
            "cbor" => "cbor",
            "msgpack" => "msgpack",
            other => return Err(PluginError::new("bin.unknown_format").with("format", other)),
        };
        let bytes = if let Some(path) = &args.path {
            ctx.stage("read");
            let size = ctx
                .file_size(path)
                .map_err(|_| PluginError::new("bin.file_read"))?;
            if size > MAX_BYTES as u64 {
                return Err(PluginError::new("bin.too_large").with("mb", MAX_BYTES >> 20));
            }
            let mut reader = ctx
                .open_file(path)
                .map_err(|_| PluginError::new("bin.file_read"))?;
            let mut data = Vec::with_capacity(size as usize);
            // 分块读取以便取消
            let mut chunk = [0u8; 65536];
            loop {
                if ctx.is_cancelled() {
                    return Err(PluginError::new("task.cancelled"));
                }
                let read = reader
                    .read(&mut chunk)
                    .map_err(|_| PluginError::new("bin.file_read"))?;
                if read == 0 {
                    break;
                }
                data.extend_from_slice(&chunk[..read]);
                if data.len() > MAX_BYTES {
                    return Err(PluginError::new("bin.too_large").with("mb", MAX_BYTES >> 20));
                }
            }
            data
        } else {
            let text = args.text.as_deref().unwrap_or_default();
            if text.len() > MAX_BYTES {
                return Err(PluginError::new("bin.too_large").with("mb", MAX_BYTES >> 20));
            }
            let encoding = args.encoding.as_deref().unwrap_or_default();
            match decode_transport(format, encoding, text) {
                Ok(bytes) => bytes,
                Err(report) => {
                    let (handle, report) = store(&self.last, report);
                    return with_handle(handle, &report);
                }
            }
        };
        if ctx.is_cancelled() {
            return Err(PluginError::new("task.cancelled"));
        }
        if bytes.is_empty() {
            let (handle, report) = store(
                &self.last,
                diag_report(format, "bin.empty", serde_json::Map::new(), None),
            );
            return with_handle(handle, &report);
        }
        ctx.stage("parse");
        let mut parse_ctx = model::Ctx::new(&bytes);
        let (root, trailing) = match format {
            "cbor" => cbor::run(&mut parse_ctx),
            _ => msgpack::run(&mut parse_ctx),
        };
        ctx.stage("done");
        let mut stats = Stats {
            nodes: parse_ctx.nodes,
            ..Stats::default()
        };
        for diag in &parse_ctx.diags {
            match diag.level {
                model::ERROR => stats.errors += 1,
                model::WARNING => stats.warnings += 1,
                _ => stats.hints += 1,
            }
        }
        let overall = if parse_ctx.incomplete {
            "incomplete"
        } else if stats.errors > 0 || trailing.is_some() {
            "issues"
        } else if stats.warnings > 0 {
            "issues"
        } else {
            "passed"
        };
        let report = Report {
            format,
            overall,
            incomplete: parse_ctx.incomplete,
            stats,
            root,
            trailing,
            diagnostics: parse_ctx.diags,
        };
        let (handle, report) = store(&self.last, report);
        with_handle(handle, &report)
    }
}

impl ToolPlugin for BinaryInspector {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "export" => {
                let args: ExportArgs = parse_args(args)?;
                let cache = self.last.lock().unwrap_or_else(|e| e.into_inner());
                let Some((_, report)) = cache.as_ref() else {
                    return Err(PluginError::new("bin.no_result"));
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

fn describe(node: &model::Node) -> String {
    let mut line = format!("{} @0x{:x}+{}", node.kind, node.offset, node.length);
    if let Some(value) = &node.value {
        let preview: String = value.chars().take(80).collect();
        line.push_str(&format!(" = {preview}"));
    }
    if let Some(number) = node.number {
        line.push_str(&format!(" (#{number})"));
    }
    if node.indefinite == Some(true) {
        line.push_str(" (indefinite)");
    }
    line
}

fn walk_markdown(node: &model::Node, out: &mut String, indent: usize) {
    if indent > 6 {
        out.push_str(&"  ".repeat(indent));
        out.push_str("…\n");
        return;
    }
    out.push_str(&format!("{}- {}\n", "  ".repeat(indent), describe(node)));
    if let Some(children) = &node.children {
        for child in children {
            walk_markdown(child, out, indent + 1);
        }
    }
    if let Some(entries) = &node.entries {
        for entry in entries {
            out.push_str(&format!("{}- key:\n", "  ".repeat(indent + 1)));
            walk_markdown(&entry.key, out, indent + 2);
            out.push_str(&format!("{}- value:\n", "  ".repeat(indent + 1)));
            walk_markdown(&entry.value, out, indent + 2);
        }
    }
}

/// Markdown 报告：结论 + 统计 + 诊断 + 类型树摘要
pub fn markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# Binary Serialization Inspection\n\n");
    out.push_str(&format!("- Format: **{}**\n", report.format));
    out.push_str(&format!("- Overall: **{}**\n", report.overall));
    if report.incomplete {
        out.push_str("- Note: a limit was hit, results are incomplete\n");
    }
    out.push_str(&format!(
        "- Nodes: {}, diagnostics: {} errors / {} warnings / {} hints\n\n",
        report.stats.nodes, report.stats.errors, report.stats.warnings, report.stats.hints
    ));
    if let Some(trailing) = &report.trailing {
        out.push_str(&format!(
            "- Trailing data: {} bytes at 0x{:x}\n",
            trailing.length, trailing.offset
        ));
    }
    if !report.diagnostics.is_empty() {
        out.push_str("## Diagnostics\n\n");
        for diag in &report.diagnostics {
            out.push_str(&format!(
                "- **{}** `{}`{}\n",
                diag.level,
                diag.code,
                diag.offset
                    .map(|offset| format!(" @0x{offset:x}"))
                    .unwrap_or_default(),
            ));
        }
        out.push('\n');
    }
    if let Some(root) = &report.root {
        out.push_str("## Value tree\n\n");
        walk_markdown(root, &mut out, 0);
    }
    out
}

tf_plugin_guest::export_plugin!(BinaryInspector);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
