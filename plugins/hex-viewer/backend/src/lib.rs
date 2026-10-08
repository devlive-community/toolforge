//! 十六进制查看插件后端：按需读取任意大小的文件并格式化为十六进制行，识别文件类型，
//! 查找字节或文本、跳转偏移，用数据检查器解读光标处的字节，并按多种格式复制选区。

mod file;
mod inspect;
mod magic;
mod search;

use std::path::Path;

use data_encoding::BASE64;
use serde::Deserialize;
use serde_json::{Value, json};
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use search::Mode;

const MANIFEST: &str = include_str!("../../manifest.json");
/// 复制选区的上限
const MAX_COPY: u64 = 1024 * 1024;

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct ReadArgs {
    path: String,
    offset: u64,
    length: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InspectArgs {
    path: String,
    offset: u64,
    #[serde(default = "yes")]
    little_endian: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindArgs {
    path: String,
    query: String,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    ignore_case: bool,
    /// 向后查找时从这里开始（含）；向前查找时找它之前的匹配
    from: u64,
    #[serde(default)]
    backward: bool,
}

#[derive(Deserialize)]
struct GotoArgs {
    expr: String,
    current: u64,
    size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Format {
    Hex,
    HexCompact,
    Base64,
    C,
    Python,
    Text,
}

#[derive(Deserialize)]
struct CopyArgs {
    path: String,
    offset: u64,
    length: u64,
    format: Format,
}

fn open(args: PathArgs) -> PluginResult<Value> {
    let (_, size) = file::open(&args.path)?;
    let head = file::read_at(&args.path, 0, 512)?;
    Ok(json!({
        "path": args.path,
        "name": Path::new(&args.path).file_name().map(|n| n.to_string_lossy().into_owned()),
        "size": size,
        "kind": magic::detect(&head),
        "offsetWidth": file::offset_width(size),
    }))
}

fn read(args: ReadArgs) -> PluginResult<Value> {
    let (_, size) = file::open(&args.path)?;
    // 按整行对齐
    let offset = args.offset - args.offset % file::ROW;
    let bytes = file::read_at(&args.path, offset, args.length)?;
    to_value(file::rows(&bytes, offset, file::offset_width(size)))
}

fn parse_number(text: &str) -> Option<u64> {
    let text = text.trim().replace('_', "");
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if let Some(hex) = text.strip_suffix('h').or_else(|| text.strip_suffix('H')) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        text.parse().ok()
    }
}

/// 解析跳转位置：十进制、0x 或 h 结尾的十六进制，+ / - 表示相对当前位置
fn goto(args: GotoArgs) -> PluginResult<Value> {
    let expr = args.expr.trim();
    let invalid = || PluginError::new("hex.invalid_offset");
    let target = if let Some(rest) = expr.strip_prefix('+') {
        args.current
            .checked_add(parse_number(rest).ok_or_else(invalid)?)
    } else if let Some(rest) = expr.strip_prefix('-') {
        args.current
            .checked_sub(parse_number(rest).ok_or_else(invalid)?)
    } else {
        Some(parse_number(expr).ok_or_else(invalid)?)
    };
    match target {
        Some(offset) if offset < args.size.max(1) => Ok(json!({ "offset": offset })),
        _ => Err(PluginError::new("hex.offset_out_of_range").with("size", args.size)),
    }
}

fn copy(args: CopyArgs) -> PluginResult<Value> {
    if args.length > MAX_COPY {
        return Err(PluginError::new("hex.copy_too_large").with("max", MAX_COPY));
    }
    let bytes = file::read_at(&args.path, args.offset, args.length)?;
    let hex = |sep: &str| {
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(sep)
    };
    let text = match args.format {
        Format::Hex => hex(" "),
        Format::HexCompact => hex(""),
        Format::Base64 => BASE64.encode(&bytes),
        Format::C => {
            let lines: Vec<String> = bytes
                .chunks(12)
                .map(|chunk| {
                    let items: Vec<String> = chunk.iter().map(|b| format!("0x{b:02x}")).collect();
                    format!("    {},", items.join(", "))
                })
                .collect();
            format!(
                "const unsigned char data[{}] = {{\n{}\n}};",
                bytes.len(),
                lines.join("\n")
            )
        }
        Format::Python => {
            let body: String = bytes.iter().map(|b| format!("\\x{b:02x}")).collect();
            format!("b'{body}'")
        }
        Format::Text => String::from_utf8_lossy(&bytes).into_owned(),
    };
    Ok(json!({ "text": text, "length": bytes.len() }))
}

pub struct HexViewer {
    manifest: Manifest,
}

impl Default for HexViewer {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for HexViewer {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "open" => open(parse_args(args)?),
            "read" => read(parse_args(args)?),
            "inspect" => {
                let args: InspectArgs = parse_args(args)?;
                let bytes = file::read_at(&args.path, args.offset, 8)?;
                to_value(inspect::values(&bytes, args.little_endian))
            }
            "find" => {
                let args: FindArgs = parse_args(args)?;
                let needle = search::pattern(&args.query, args.mode)?;
                let ignore_case = args.ignore_case && args.mode == Mode::Text;
                let never = || false;
                let found = if args.backward {
                    search::backward(&args.path, &needle, args.from, ignore_case, &never)?
                } else {
                    search::forward(&args.path, &needle, args.from, ignore_case, &never)?
                };
                Ok(json!({ "offset": found, "length": needle.len() }))
            }
            "goto" => goto(parse_args(args)?),
            "copy" => copy(parse_args(args)?),
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
