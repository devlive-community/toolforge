//! 表格转换插件后端：自动识别 Markdown / HTML / JSON / TSV / CSV 表格，可设置表头、对齐、转置，
//! 输出为 Markdown（按中日韩字符宽度对齐）、CSV、TSV、HTML、JSON、ASCII 边框表、SQL INSERT 与 LaTeX。

mod parse;
mod render;
mod table;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{Manifest, PluginResult, ToolPlugin, parse_args, to_value, unknown_function};

use parse::Source;
use render::Target;
use table::{Align, Table};

const MANIFEST: &str = include_str!("../../manifest.json");
/// 预览最多返回的行数
const PREVIEW_ROWS: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvertArgs {
    input: String,
    /// 为空时自动识别
    #[serde(default)]
    from: Option<Source>,
    to: Target,
    /// 为空时：Markdown / HTML / JSON 对象数组按原有表头，其余把第一行当表头
    #[serde(default)]
    header: Option<bool>,
    #[serde(default)]
    transpose: bool,
    #[serde(default = "yes")]
    trim: bool,
    /// 每列的对齐方式；为空时保留输入中的设置
    #[serde(default)]
    aligns: Option<Vec<Align>>,
    #[serde(flatten)]
    options: render::Options,
}

fn yes() -> bool {
    true
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Converted {
    output: String,
    detected: Source,
    header: bool,
    columns: usize,
    rows: usize,
    preview: Table,
    truncated: bool,
}

fn convert(args: ConvertArgs) -> PluginResult<Converted> {
    let detected = args.from.unwrap_or_else(|| parse::detect(&args.input));
    let mut table = parse::parse(&args.input, detected)?;
    let had_header = !table.headers.is_empty();
    let header = args
        .header
        .unwrap_or(had_header || matches!(detected, Source::Csv | Source::Tsv));
    table = table.with_header(header);
    if args.trim {
        table = table.trim();
    }
    table = table.normalize();
    if args.transpose {
        table = table.transpose();
    }
    if let Some(aligns) = args.aligns {
        table.aligns = aligns;
        table = table.normalize();
    }
    let output = render::render(&table, args.to, &args.options);
    let mut preview = table.clone();
    let truncated = preview.rows.len() > PREVIEW_ROWS;
    preview.rows.truncate(PREVIEW_ROWS);
    Ok(Converted {
        output,
        detected,
        header,
        columns: table.columns(),
        rows: table.rows.len(),
        preview,
        truncated,
    })
}

pub struct TableConverter {
    manifest: Manifest,
}

impl Default for TableConverter {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for TableConverter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        // 从表格软件复制的内容：多行且每行都有制表符
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.len() >= 2 && lines.iter().all(|l| l.contains('\t')) {
            return Some(tf_plugin_api::Detection::new(60, "tsv").with("rows", lines.len()));
        }
        (parse::detect(text) == Source::Markdown && lines.len() >= 2)
            .then(|| tf_plugin_api::Detection::new(70, "markdown").with("rows", lines.len() - 2))
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
