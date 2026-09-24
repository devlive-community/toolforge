use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tf_plugin_api::{PluginError, PluginResult};

use crate::csvfmt::{self, Delimiter};
use crate::tomlfmt;

const MAX_INPUT: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Auto,
    Json,
    Yaml,
    Toml,
    Csv,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    #[serde(default = "two")]
    pub indent: usize,
    #[serde(default)]
    pub minify: bool,
    #[serde(default = "comma")]
    pub delimiter: Delimiter,
    #[serde(default = "yes")]
    pub header: bool,
    #[serde(default = "yes")]
    pub infer_types: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            indent: 2,
            minify: false,
            delimiter: Delimiter::Comma,
            header: true,
            infer_types: true,
        }
    }
}

fn two() -> usize {
    2
}

fn comma() -> Delimiter {
    Delimiter::Comma
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
pub struct Args {
    pub input: String,
    pub from: Format,
    pub to: Format,
    #[serde(default)]
    pub options: Options,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    /// 实际使用的输入格式（from 为 auto 时为检测结果）
    pub from: Format,
    /// 顶层数组的元素数（CSV 为行数）
    pub records: Option<usize>,
    pub elapsed_ms: f64,
}

/// 字节偏移 → 1 起始的行列号
fn line_col(input: &str, offset: usize) -> (usize, usize) {
    let before = &input[..input.floor_char_boundary(offset.min(input.len()))];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|l| l.chars().count())
        .unwrap_or(0)
        + 1;
    (line, column)
}

/// 猜测输入格式：{ / [ 开头为 JSON，TOML 表头或 key = value 为 TOML，
/// 首行含分隔符且不像 YAML 映射时为 CSV，其余视为 YAML
pub fn detect(input: &str) -> Format {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{')
        || trimmed.starts_with('[') && serde_json::from_str::<Value>(trimmed).is_ok()
    {
        return Format::Json;
    }
    let first = trimmed
        .lines()
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .unwrap_or("");
    let is_toml_line = |l: &str| {
        let l = l.trim();
        (l.starts_with('[') && l.ends_with(']'))
            || l.split_once('=')
                .is_some_and(|(k, _)| !k.trim().is_empty() && !k.contains(':'))
    };
    if is_toml_line(first) && toml::from_str::<toml::Table>(input).is_ok() {
        return Format::Toml;
    }
    let looks_yaml = first.contains(": ")
        || first.trim_end().ends_with(':')
        || first.starts_with("- ")
        || first == "---";
    if !looks_yaml && (first.contains(',') || first.contains('\t') || first.contains(';')) {
        return Format::Csv;
    }
    Format::Yaml
}

fn parse(input: &str, format: Format, options: &Options) -> PluginResult<Value> {
    match format {
        Format::Json | Format::Auto => serde_json::from_str(input).map_err(|e| {
            PluginError::new("format.json_invalid")
                .with("line", e.line())
                .with("column", e.column())
                .with("detail", e.to_string())
        }),
        Format::Yaml => {
            let mut documents = Vec::new();
            for document in serde_yaml_ng::Deserializer::from_str(input) {
                let value = Value::deserialize(document).map_err(|e| {
                    let mut err =
                        PluginError::new("format.yaml_invalid").with("detail", e.to_string());
                    if let Some(location) = e.location() {
                        err = err
                            .with("line", location.line())
                            .with("column", location.column());
                    }
                    err
                })?;
                documents.push(value);
            }
            // 多文档 YAML 转为数组
            Ok(match documents.len() {
                0 => Value::Null,
                1 => documents.pop().expect("one document"),
                _ => Value::Array(documents),
            })
        }
        Format::Toml => {
            let table: toml::Table = toml::from_str(input).map_err(|e| {
                let mut err =
                    PluginError::new("format.toml_invalid").with("detail", e.message().to_owned());
                if let Some(span) = e.span() {
                    let (line, column) = line_col(input, span.start);
                    err = err.with("line", line).with("column", column);
                }
                err
            })?;
            Ok(tomlfmt::to_json(toml::Value::Table(table)))
        }
        Format::Csv => csvfmt::parse(
            input,
            options.delimiter,
            options.header,
            options.infer_types,
        ),
    }
}

fn serialize(value: &Value, format: Format, options: &Options) -> PluginResult<String> {
    let failed = |e: String| PluginError::new("format.serialize_failed").with("detail", e);
    match format {
        Format::Json | Format::Auto => {
            if options.minify {
                return serde_json::to_string(value).map_err(|e| failed(e.to_string()));
            }
            let indent = vec![b' '; options.indent.clamp(1, 8)];
            let mut out = Vec::new();
            let mut ser = serde_json::Serializer::with_formatter(
                &mut out,
                serde_json::ser::PrettyFormatter::with_indent(&indent),
            );
            value
                .serialize(&mut ser)
                .map_err(|e| failed(e.to_string()))?;
            String::from_utf8(out).map_err(|e| failed(e.to_string()))
        }
        Format::Yaml => {
            serde_yaml_ng::to_string(&to_yaml(value)).map_err(|e| failed(e.to_string()))
        }
        Format::Toml => {
            if !value.is_object() {
                return Err(PluginError::new("format.toml_root"));
            }
            let table = tomlfmt::from_json(value, &mut Vec::new())?;
            toml::to_string_pretty(&table).map_err(|e| failed(e.to_string()))
        }
        Format::Csv => csvfmt::write(value, options.delimiter).map(|(text, _)| text),
    }
}

/// 工作区启用了 serde_json 的 arbitrary_precision，数字经其他 serde 格式序列化时
/// 会变成私有结构，因此显式转换为 YAML 值
fn to_yaml(value: &Value) -> serde_yaml_ng::Value {
    use serde_yaml_ng::{Mapping, Number, Value as Yaml};
    match value {
        Value::Null => Yaml::Null,
        Value::Bool(b) => Yaml::Bool(*b),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => Yaml::Number(Number::from(i)),
            (None, Some(u), _) => Yaml::Number(Number::from(u)),
            (None, None, Some(f)) => Yaml::Number(Number::from(f)),
            _ => Yaml::String(n.to_string()),
        },
        Value::String(s) => Yaml::String(s.clone()),
        Value::Array(items) => Yaml::Sequence(items.iter().map(to_yaml).collect()),
        Value::Object(map) => {
            let mut mapping = Mapping::new();
            for (key, item) in map {
                mapping.insert(Yaml::String(key.clone()), to_yaml(item));
            }
            Yaml::Mapping(mapping)
        }
    }
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.input.trim().is_empty() {
        return Err(PluginError::new("format.empty"));
    }
    if args.input.len() > MAX_INPUT {
        return Err(PluginError::new("format.too_large").with("limit", "10 MB"));
    }
    let start = Instant::now();
    let from = if args.from == Format::Auto {
        detect(&args.input)
    } else {
        args.from
    };
    let value = parse(&args.input, from, &args.options)?;
    let output = serialize(&value, args.to, &args.options)?;
    Ok(Output {
        output,
        from,
        records: value.as_array().map(Vec::len),
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
