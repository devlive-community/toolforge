//! 模拟数据插件后端：按字段定义生成逼真的中文 / 英文数据，输出为 JSON、CSV 或 SQL。

mod data;
mod generate;
mod output;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

/// 预览最多生成的行数；保存时用同一个种子生成全部数据，预览正好是它的开头
pub const PREVIEW_ROWS: usize = 200;

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct GenerateArgs {
    #[serde(flatten)]
    schema: generate::Schema,
    #[serde(flatten)]
    options: output::Options,
}

#[derive(Deserialize)]
struct SaveArgs {
    #[serde(flatten)]
    schema: generate::Schema,
    #[serde(flatten)]
    options: output::Options,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Preview {
    output: String,
    seed: u64,
    rows: usize,
    preview_rows: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    path: String,
    rows: usize,
    bytes: usize,
}

fn today() -> jiff::civil::Date {
    jiff::Zoned::now().date()
}

fn check_table(options: &output::Options) -> PluginResult<()> {
    if options.format == output::Format::Sql && options.table.trim().is_empty() {
        return Err(PluginError::new("mock.no_table"));
    }
    Ok(())
}

fn preview(args: GenerateArgs) -> PluginResult<Preview> {
    check_table(&args.options)?;
    let data = generate::generate(&args.schema, Some(PREVIEW_ROWS), today())?;
    Ok(Preview {
        output: output::render(&data, &args.options),
        seed: data.seed,
        rows: args.schema.rows,
        preview_rows: data.rows.len(),
    })
}

fn save(args: SaveArgs) -> PluginResult<Saved> {
    check_table(&args.options)?;
    if args.schema.seed.is_none() {
        return Err(PluginError::new("mock.no_seed"));
    }
    let data = generate::generate(&args.schema, None, today())?;
    let text = output::render(&data, &args.options);
    std::fs::write(&args.path, &text).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", args.path.as_str())
            .with("detail", e.to_string())
    })?;
    Ok(Saved {
        path: args.path,
        rows: data.rows.len(),
        bytes: text.len(),
    })
}

pub struct MockData {
    manifest: Manifest,
}

impl Default for MockData {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for MockData {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(preview(parse_args(args)?)?),
            "save" => to_value(save(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
