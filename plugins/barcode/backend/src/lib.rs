//! 条形码插件后端：生成 EAN-13 / EAN-8 / UPC-A / Code 128 / Code 39 / Code 93 / ITF / Codabar
//! 以及 Data Matrix / PDF417 / Aztec，自动补全校验位，导出 PNG / SVG；识别图片中的各种条码。

mod decode;
mod formats;
mod render;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use formats::Format;
use render::Style;

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateArgs {
    format: Format,
    text: String,
    #[serde(flatten)]
    style: Style,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum SaveFormat {
    Png,
    Svg,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveArgs {
    #[serde(flatten)]
    barcode: GenerateArgs,
    save_as: SaveFormat,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Generated {
    svg: String,
    width: u32,
    height: u32,
    /// 实际编码的内容（含校验位）
    content: String,
    check_digit: Option<char>,
    notes: Vec<String>,
}

fn build(args: &GenerateArgs) -> PluginResult<(formats::Prepared, render::Drawing)> {
    let prepared = formats::prepare(args.format, &args.text)?;
    let symbol = render::encode(args.format, &prepared.content)?;
    let drawing = render::draw(args.format, &symbol, &prepared.content, &args.style)?;
    Ok((prepared, drawing))
}

fn generate(args: GenerateArgs) -> PluginResult<Generated> {
    let (prepared, drawing) = build(&args)?;
    Ok(Generated {
        svg: drawing.svg,
        width: drawing.pixels.0,
        height: drawing.pixels.1,
        content: prepared.content,
        check_digit: prepared.check_digit,
        notes: prepared.notes,
    })
}

fn save(args: SaveArgs) -> PluginResult<()> {
    let (_, drawing) = build(&args.barcode)?;
    let bytes = match args.save_as {
        SaveFormat::Png => render::png(&drawing)?,
        SaveFormat::Svg => drawing.svg.into_bytes(),
    };
    std::fs::write(&args.path, bytes).map_err(|e| {
        PluginError::new("fs.io")
            .with("path", args.path.as_str())
            .with("detail", e.to_string())
    })
}

pub struct BarcodeTool {
    manifest: Manifest,
}

impl Default for BarcodeTool {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for BarcodeTool {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// 复制的 8 / 12 / 13 位数字且校验位正确时，可能是商品条码
    fn detect(&self, text: &str) -> Option<tf_plugin_api::Detection> {
        let text = text.trim();
        let format = match text.len() {
            8 => "ean8",
            12 => "upcA",
            13 => "ean13",
            _ => return None,
        };
        let (body, check) = text.split_at(text.len() - 1);
        let valid = text.bytes().all(|b| b.is_ascii_digit())
            && check.starts_with(formats::gs1_check_digit(body));
        // 13 位数字也可能是毫秒时间戳，分数放低一些
        valid.then(|| tf_plugin_api::Detection::new(50, format))
    }

    /// 剪贴板中的图片（如截图）可能包含条码
    fn detect_image(&self, width: u32, height: u32) -> Option<tf_plugin_api::Detection> {
        (width.min(height) >= 20).then(|| tf_plugin_api::Detection::new(60, "image"))
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "generate" => to_value(generate(parse_args(args)?)?),
            "save" => to_value(save(parse_args(args)?)?),
            "decode" => to_value(decode::decode(parse_args(args)?)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
