//! JSON 转代码类型插件后端：从 JSON 样例推断结构，生成多种语言的类型定义。

mod detect;
mod emit;
mod model;
mod schema;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");
const MAX_INPUT: usize = 10 * 1024 * 1024;

fn default_root() -> String {
    "Root".into()
}

#[derive(Deserialize)]
struct Args {
    input: String,
    language: emit::Language,
    #[serde(default = "default_root")]
    root: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    code: String,
    language: &'static str,
    types: usize,
}

fn generate(args: Args) -> PluginResult<Output> {
    let input = args.input.trim();
    if input.is_empty() {
        return Err(PluginError::new("code.empty"));
    }
    if input.len() > MAX_INPUT {
        return Err(PluginError::new("code.too_large").with("limit", "10 MB"));
    }
    let value: Value = serde_json::from_str(input).map_err(|e| {
        PluginError::new("code.invalid_json")
            .with("line", e.line())
            .with("column", e.column())
            .with("detail", e.to_string())
    })?;
    let root = if args.root.trim().is_empty() {
        default_root()
    } else {
        model::pascal(&args.root)
    };
    let model = model::build(&schema::infer(&value), &root);
    Ok(Output {
        code: emit::emit(&model, args.language, &emit::Options { root }),
        language: args.language.id(),
        types: model.structs.len(),
    })
}

pub struct JsonToCode {
    manifest: Manifest,
}

impl Default for JsonToCode {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for JsonToCode {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        detect::detect(text)
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            "languages" => to_value(emit::LANGUAGES.map(|l| l.id())),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
