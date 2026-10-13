//! .env 编辑器插件后端：打开、编辑与保存 dotenv 文件，解析与校验都在本机完成。

mod document;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

const MANIFEST: &str = include_str!("../../manifest.json");

pub struct EnvEditor {
    manifest: Manifest,
}

impl Default for EnvEditor {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveArgs {
    path: String,
    lines: Vec<document::Line>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenResult {
    path: String,
    lines: Vec<document::Line>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveResult {
    bytes: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExampleResult {
    path: String,
}

/// 打开并解析文件；值统一转为单行显示转义，UI 编辑后再保存
fn open_file(path: &str) -> PluginResult<OpenResult> {
    let meta = std::fs::metadata(path).map_err(|e| {
        PluginError::new("env.io")
            .with("path", path)
            .with("detail", e.to_string())
    })?;
    if meta.len() > document::MAX_FILE {
        return Err(PluginError::new("env.file_too_large").with("limit", "1 MB"));
    }
    let text = std::fs::read_to_string(path).map_err(|e| {
        PluginError::new("env.io")
            .with("path", path)
            .with("detail", e.to_string())
    })?;
    let lines = document::parse(&text)
        .into_iter()
        .map(|line| match line {
            document::Line::Entry {
                export,
                key,
                value,
                quote,
                inline_comment,
                multiline,
            } => document::Line::Entry {
                export,
                key,
                value: document::escape_display(&value),
                quote,
                inline_comment,
                multiline,
            },
            other => other,
        })
        .collect();
    Ok(OpenResult {
        path: path.to_owned(),
        lines,
    })
}

/// 把 UI 传回的显示转义值还原为真实值
fn unescape_lines(lines: Vec<document::Line>) -> Vec<document::Line> {
    lines
        .into_iter()
        .map(|line| match line {
            document::Line::Entry {
                export,
                key,
                value,
                quote,
                inline_comment,
                multiline,
            } => document::Line::Entry {
                export,
                key,
                value: document::unescape_double(&value),
                quote,
                inline_comment,
                multiline,
            },
            other => other,
        })
        .collect()
}

fn save_file(args: SaveArgs) -> PluginResult<SaveResult> {
    let lines = unescape_lines(args.lines);
    document::validate(&lines)?;
    let text = document::serialize(&lines);
    std::fs::write(&args.path, &text).map_err(|e| {
        PluginError::new("env.io")
            .with("path", args.path.as_str())
            .with("detail", e.to_string())
    })?;
    Ok(SaveResult {
        bytes: text.len() as u64,
    })
}

fn export_example(args: SaveArgs) -> PluginResult<ExampleResult> {
    let lines = unescape_lines(args.lines);
    document::validate(&lines)?;
    let text = document::to_example_text(&lines);
    let output = format!("{}.example", args.path);
    if std::path::Path::new(&output).exists() {
        return Err(PluginError::new("env.example_exists").with("path", output.as_str()));
    }
    std::fs::write(&output, &text).map_err(|e| {
        PluginError::new("env.io")
            .with("path", output.as_str())
            .with("detail", e.to_string())
    })?;
    Ok(ExampleResult { path: output })
}

impl ToolPlugin for EnvEditor {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "open" => to_value(open_file(&parse_args::<PathArgs>(args)?.path)?),
            "save" => to_value(save_file(parse_args::<SaveArgs>(args)?)?),
            "export_example" => to_value(export_example(parse_args::<SaveArgs>(args)?)?),
            _ => Err(unknown_function(function)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
