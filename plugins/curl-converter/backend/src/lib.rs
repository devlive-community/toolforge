//! cURL 转代码插件后端：解析 curl 命令（bash / cmd 写法），生成多种语言的请求代码。

mod detect;
mod emit;
mod parse;
mod tokenize;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use parse::{Request, Warning};

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_INPUT: usize = 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    command: String,
    language: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Converted {
    code: String,
    request: Request,
    warnings: Vec<Warning>,
}

fn convert(args: Args) -> PluginResult<Converted> {
    if args.command.len() > MAX_INPUT {
        return Err(PluginError::new("curl.too_large").with("limit", "1 MB"));
    }
    if args.command.trim().is_empty() {
        return Err(PluginError::new("curl.empty"));
    }
    let tokens = tokenize::tokenize(&args.command)?;
    let (request, warnings) = parse::parse(&tokens)?;
    let code = match args.language.as_str() {
        "javascript" => emit::javascript(&request),
        "python" => emit::python(&request),
        "go" => emit::go(&request),
        "rust" => emit::rust(&request),
        "java" => emit::java(&request),
        "php" => emit::php(&request),
        "csharp" => emit::csharp(&request),
        other => {
            return Err(PluginError::new("curl.unknown_language").with("language", other));
        }
    };
    Ok(Converted {
        code,
        request,
        warnings,
    })
}

pub struct CurlConverter {
    manifest: Manifest,
}

impl Default for CurlConverter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for CurlConverter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "convert" => to_value(convert(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
