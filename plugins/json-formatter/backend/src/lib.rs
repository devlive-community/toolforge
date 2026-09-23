//! JSON 格式化插件后端：解析、格式化、压缩、校验、转义、树形浏览与对比，
//! 所有数据处理都在这里完成，前端只负责展示。

mod diff;
mod docs;
mod format;
mod parse;
mod stats;
mod tree;

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

pub use parse::Mode;

use docs::DocCache;
use stats::Stats;

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct JsonFormatter {
    manifest: Manifest,
    docs: DocCache,
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
            docs: DocCache::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceArgs {
    input: String,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    indent: format::Indent,
    #[serde(default)]
    sort_keys: bool,
}

#[derive(Deserialize)]
struct TextArgs {
    input: String,
}

/// 处理结果：输出文本、可供树形浏览的文档 id、统计信息与耗时
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    output: String,
    doc_id: Option<u64>,
    stats: Option<Stats>,
    elapsed_ms: f64,
}

fn elapsed(start: Instant) -> f64 {
    (start.elapsed().as_secs_f64() * 1000.0 * 100.0).round() / 100.0
}

impl JsonFormatter {
    fn render(&self, args: SourceArgs, minify: bool) -> PluginResult<Output> {
        let start = Instant::now();
        let mut value = parse::parse(&args.input, args.mode)?;
        if args.sort_keys {
            format::sort_keys(&mut value);
        }
        let output = if minify {
            format::minify(&value)?
        } else {
            format::pretty(&value, args.indent)?
        };
        let stats = Stats::collect(&value, &output);
        let doc_id = self.docs.insert(value);
        Ok(Output {
            output,
            doc_id: Some(doc_id),
            stats: Some(stats),
            elapsed_ms: elapsed(start),
        })
    }

    fn text(&self, args: TextArgs, f: fn(&str) -> PluginResult<String>) -> PluginResult<Output> {
        let start = Instant::now();
        let output = f(&args.input)?;
        Ok(Output {
            output,
            doc_id: None,
            stats: None,
            elapsed_ms: elapsed(start),
        })
    }
}

impl ToolPlugin for JsonFormatter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "format" => to_value(self.render(parse_args(args)?, false)?),
            "minify" => to_value(self.render(parse_args(args)?, true)?),
            // 校验与格式化共享解析流程，输出格式化结果便于定位
            "validate" => to_value(self.render(parse_args(args)?, false)?),
            "escape" => to_value(self.text(parse_args(args)?, format::escape)?),
            "unescape" => to_value(self.text(parse_args(args)?, format::unescape)?),
            "tree_children" => to_value(tree::children(&self.docs, parse_args(args)?)?),
            "diff" => to_value(diff::run(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
