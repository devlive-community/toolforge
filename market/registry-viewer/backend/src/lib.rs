//! 注册表导出查看器：只读解析 .reg 文本与两份对比，不碰本机注册表。

mod diff;
mod doc;
mod value;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, TaskContext, ToolPlugin, parse_args, to_value,
    unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 单文件字节上限
const MAX_FILE: u64 = 32 << 20;

pub struct RegistryViewer {
    manifest: Manifest,
}

impl Default for RegistryViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathArgs {
    path: String,
}

/// 读文件（guest 读取，任意调用可用）并解码校验头
fn load(path: &str, ctx: Option<&dyn TaskContext>) -> PluginResult<String> {
    let size = match ctx {
        Some(ctx) => ctx.file_size(path)?,
        None => tf_plugin_guest::net::file_size(path)?,
    };
    if size > MAX_FILE {
        return Err(errp("reg.file_too_large", "limit", "32 MB"));
    }
    let bytes = match ctx {
        Some(ctx) => {
            let mut bytes = Vec::with_capacity(size as usize);
            ctx.open_file(path)?
                .read_to_end(&mut bytes)
                .map_err(|e| errp("reg.io", "path", path).with("detail", e.to_string()))?;
            bytes
        }
        None => tf_plugin_guest::net::read_file(path, 0, size.min(u32::MAX as u64) as u32)?,
    };
    doc::decode_text(&bytes)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParseOut {
    doc: doc::RegDoc,
    /// 值总数
    value_count: usize,
}

/// 解析 .reg 文件（键树 + 源文行号定位）
fn parse_file(args: PathArgs, ctx: Option<&dyn TaskContext>) -> PluginResult<ParseOut> {
    let text = load(&args.path, ctx)?;
    let doc = doc::parse(&text)?;
    let value_count = doc.keys.iter().map(|k| k.values.len()).sum();
    Ok(ParseOut { doc, value_count })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffArgs {
    path_a: String,
    path_b: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffOut {
    #[serde(flatten)]
    report: diff::DiffReport,
    doc_a: doc::RegDoc,
    doc_b: doc::RegDoc,
}

/// 解析两份并对比；任务内按键取消
fn diff_files(args: DiffArgs, ctx: &dyn TaskContext) -> PluginResult<DiffOut> {
    let text_a = load(&args.path_a, Some(ctx))?;
    let doc_a = doc::parse(&text_a)?;
    let text_b = load(&args.path_b, Some(ctx))?;
    let doc_b = doc::parse(&text_b)?;
    let report = diff::diff(&doc_a, &doc_b, Some(ctx))?;
    Ok(DiffOut {
        report,
        doc_a,
        doc_b,
    })
}

impl ToolPlugin for RegistryViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "parse" => to_value(parse_file(parse_args(args)?, None)?),
            other => Err(unknown_function(other)),
        }
    }

    fn run_task(&self, function: &str, args: Value, ctx: &dyn TaskContext) -> PluginResult<Value> {
        match function {
            "diff" => to_value(diff_files(parse_args(args)?, ctx)?),
            other => self.call(other, args),
        }
    }
}

tf_plugin_guest::export_plugin!(RegistryViewer);

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
