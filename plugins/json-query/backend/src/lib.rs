//! JSON 查询插件后端：用 jq（jaq 实现）或 JSONPath（RFC 9535）查询 JSON 文本或文件，
//! 支持 JSON Lines 输入、原始字符串输出、紧凑输出与按键排序。

mod jq;
mod jsonpath;
mod source;

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

use source::Source;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 最多返回的结果个数与总字符数
const MAX_RESULTS: usize = 5_000;
const MAX_CHARS: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Language {
    #[default]
    Jq,
    Jsonpath,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// 字符串结果不加引号（jq -r）
    #[serde(default)]
    pub raw: bool,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub sort_keys: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Output {
    /// JSONPath 结果的规范化路径
    pub path: Option<String>,
    pub text: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub results: Vec<Output>,
    pub truncated: bool,
    #[serde(skip)]
    chars: usize,
}

impl Results {
    /// 加入一个结果；超过上限时返回 false
    pub fn push(&mut self, output: Output) -> bool {
        if self.results.len() >= MAX_RESULTS || self.chars + output.text.len() > MAX_CHARS {
            self.truncated = true;
            return false;
        }
        self.chars += output.text.len();
        self.results.push(output);
        true
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryArgs {
    source: Source,
    query: String,
    #[serde(default)]
    language: Language,
    #[serde(flatten)]
    options: Options,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Answer {
    #[serde(flatten)]
    results: Results,
    /// 输入中 JSON 值的个数（JSON Lines 时大于 1）
    inputs: usize,
    elapsed_ms: u64,
}

fn query(args: QueryArgs) -> PluginResult<Answer> {
    let text = source::load(&args.source)?;
    let inputs = source::validate(&text)?;
    let started = Instant::now();
    let results = match args.language {
        Language::Jq => jq::run(&args.query, text, args.options)?,
        Language::Jsonpath => jsonpath::run(&args.query, &text, &args.options)?,
    };
    Ok(Answer {
        results,
        inputs,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

pub struct JsonQuery {
    manifest: Manifest,
}

impl Default for JsonQuery {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for JsonQuery {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "query" => to_value(query(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
