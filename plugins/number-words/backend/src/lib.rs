//! 数字大写插件后端：识别输入（阿拉伯数字、中文数字、人民币大写、罗马数字），
//! 转换为人民币大写、中文大小写数字、英文读法与支票写法、罗马数字及各种分组格式。

mod chinese;
mod english;
mod number;
mod roman;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{
    Manifest, PluginError, PluginResult, ToolPlugin, parse_args, to_value, unknown_function,
};

use chinese::Case;
use number::Number;

const MANIFEST: &str = include_str!("../../manifest.json");

#[derive(Deserialize)]
struct ConvertArgs {
    input: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Source {
    Arabic,
    Chinese,
    Rmb,
    Roman,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Output {
    /// 输出类型，前端翻译标签
    kind: &'static str,
    value: Option<String>,
    /// 为什么没有结果，或结果经过了四舍五入
    note: Option<PluginError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Converted {
    source: Source,
    /// 识别出的数值（普通写法）
    value: String,
    outputs: Vec<Output>,
}

fn parse(input: &str) -> PluginResult<(Number, Source)> {
    if input.trim().is_empty() {
        return Err(PluginError::new("num.empty"));
    }
    if let Some(n) = number::parse_arabic(input) {
        return Ok((n?, Source::Arabic));
    }
    if let Some(n) = roman::parse(input) {
        return Ok((Number::new(false, &n.to_string(), "")?, Source::Roman));
    }
    if let Some(parsed) = chinese::parse(input) {
        let (n, money) = parsed?;
        return Ok((n, if money { Source::Rmb } else { Source::Chinese }));
    }
    Err(PluginError::new("num.unrecognized").with("input", input.trim()))
}

fn output(kind: &'static str, result: PluginResult<String>) -> Output {
    match result {
        Ok(value) => Output {
            kind,
            value: Some(value),
            note: None,
        },
        Err(note) => Output {
            kind,
            value: None,
            note: Some(note),
        },
    }
}

/// 四舍五入过的结果附上提示
fn rounded(kind: &'static str, result: PluginResult<(String, bool)>) -> Output {
    match result {
        Ok((value, changed)) => Output {
            kind,
            value: Some(value),
            note: changed.then(|| PluginError::new("num.rounded_cents")),
        },
        Err(note) => Output {
            kind,
            value: None,
            note: Some(note),
        },
    }
}

fn bytes(n: &Number) -> PluginResult<String> {
    if n.negative || !n.is_integer() {
        return Err(PluginError::new("num.bytes_integer"));
    }
    let value = n.to_f64();
    let pick = |base: f64, units: [&str; 7]| {
        let mut v = value;
        let mut i = 0;
        while v >= base && i < units.len() - 1 {
            v /= base;
            i += 1;
        }
        if i == 0 {
            format!("{value} {}", units[0])
        } else {
            format!(
                "{} {}",
                format!("{v:.2}")
                    .trim_end_matches('0')
                    .trim_end_matches('.'),
                units[i]
            )
        }
    };
    Ok(format!(
        "{} · {}",
        pick(1024.0, ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]),
        pick(1000.0, ["B", "kB", "MB", "GB", "TB", "PB", "EB"])
    ))
}

fn convert(input: &str) -> PluginResult<Converted> {
    let (n, source) = parse(input)?;
    let roman = || {
        let value = n.int_value().filter(|_| n.is_integer() && !n.negative);
        value
            .and_then(|v| u32::try_from(v).ok())
            .and_then(roman::to_roman)
            .ok_or_else(|| PluginError::new("num.roman_range"))
    };
    let outputs = vec![
        rounded("rmb", chinese::rmb(&n)),
        output("chineseLower", chinese::numerals(&n, Case::Lower)),
        output("chineseUpper", chinese::numerals(&n, Case::Upper)),
        output("english", english::words(&n)),
        rounded("check", english::check(&n)),
        output("roman", roman()),
        output("thousands", Ok(n.grouped(3, ','))),
        output("indian", Ok(n.indian())),
        output("spaced", Ok(n.grouped(3, ' '))),
        output("scientific", Ok(n.scientific())),
        output("bytes", bytes(&n)),
    ];
    Ok(Converted {
        source,
        value: n.plain(),
        outputs,
    })
}

pub struct NumberWords {
    manifest: Manifest,
}

impl Default for NumberWords {
    fn default() -> Self {
        Self {
            manifest: Manifest::from_static(MANIFEST),
        }
    }
}

impl ToolPlugin for NumberWords {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call(&self, function: &str, args: Value) -> PluginResult<Value> {
        match function {
            "convert" => {
                let args: ConvertArgs = parse_args(args)?;
                to_value(convert(&args.input)?)
            }
            other => Err(unknown_function(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;
