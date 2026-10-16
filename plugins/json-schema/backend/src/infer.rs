//! 从 JSON 样例推断 Schema：合并多个样例的类型、必填字段、数组元素结构与字符串格式。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tf_plugin_api::{PluginError, PluginResult};

use crate::guard;
use crate::validate::Draft;

/// 字符串取值不超过这么多种、且出现次数足够多时推断为 enum
const ENUM_MAX: usize = 5;
const ENUM_MIN_OCCURRENCES: usize = 3;
const MAX_SAMPLES: usize = 10_000;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    /// 整个输入是一个样例
    #[default]
    Document,
    /// 顶层数组中的每一项是一个样例
    Array,
    /// 每行一个 JSON（JSON Lines）
    Lines,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    pub input: String,
    #[serde(default)]
    pub split: Split,
    #[serde(default)]
    pub draft: Draft,
    #[serde(default = "yes")]
    pub enums: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub schema: String,
    pub samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Format {
    DateTime,
    Date,
    Time,
    Email,
    Uri,
    Uuid,
    Ipv4,
    Ipv6,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::DateTime => "date-time",
            Format::Date => "date",
            Format::Time => "time",
            Format::Email => "email",
            Format::Uri => "uri",
            Format::Uuid => "uuid",
            Format::Ipv4 => "ipv4",
            Format::Ipv6 => "ipv6",
        }
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    matches!(parts.as_slice(), [y, m, d] if y.len() == 4 && m.len() == 2 && d.len() == 2
        && digits(y) && digits(m) && digits(d)
        && (1..=12).contains(&m.parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&d.parse::<u32>().unwrap_or(0)))
}

/// RFC 3339 full-time：hh:mm:ss[.frac](Z|±hh:mm)
fn is_time(s: &str) -> bool {
    let (clock, offset) = if let Some(clock) = s.strip_suffix(['Z', 'z']) {
        (clock, "")
    } else if let Some(index) = s.rfind(['+', '-']) {
        (&s[..index], &s[index + 1..])
    } else {
        return false;
    };
    if !offset.is_empty() && !(offset.len() == 5 && offset.as_bytes()[2] == b':') {
        return false;
    }
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, "0"));
    let parts: Vec<&str> = hms.split(':').collect();
    parts.len() == 3 && parts.iter().all(|p| p.len() == 2 && digits(p)) && digits(frac)
}

fn detect_format(s: &str) -> Option<Format> {
    if let Some((date, time)) = s.split_once(['T', 't', ' '])
        && is_date(date)
        && is_time(time)
    {
        return Some(Format::DateTime);
    }
    if is_date(s) {
        return Some(Format::Date);
    }
    if is_time(s) {
        return Some(Format::Time);
    }
    if s.len() == 36
        && s.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        return Some(Format::Uuid);
    }
    if let Some((local, domain)) = s.split_once('@')
        && !local.is_empty()
        && !local.contains(char::is_whitespace)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains(['@', ' '])
    {
        return Some(Format::Email);
    }
    if let Some((scheme, rest)) = s.split_once("://")
        && !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !rest.is_empty()
        && !s.contains(char::is_whitespace)
    {
        return Some(Format::Uri);
    }
    if s.parse::<std::net::Ipv4Addr>().is_ok() {
        return Some(Format::Ipv4);
    }
    if s.contains(':') && s.parse::<std::net::Ipv6Addr>().is_ok() {
        return Some(Format::Ipv6);
    }
    None
}

/// 某个位置上观察到的所有取值的合并结果
#[derive(Default)]
struct Shape {
    nulls: usize,
    booleans: usize,
    integers: usize,
    numbers: usize,
    strings: usize,
    /// 所有字符串共同的格式；出现不一致时为 None
    format: Option<Option<Format>>,
    values: BTreeSet<String>,
    too_many_values: bool,
    arrays: usize,
    items: Option<Box<Shape>>,
    objects: usize,
    /// 键 → (出现次数, 结构)，保持首次出现的顺序
    properties: Vec<(String, usize, Shape)>,
}

impl Shape {
    fn add(&mut self, value: &Value) {
        match value {
            Value::Null => self.nulls += 1,
            Value::Bool(_) => self.booleans += 1,
            Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    self.integers += 1;
                } else {
                    self.numbers += 1;
                }
            }
            Value::String(s) => {
                self.strings += 1;
                let format = detect_format(s);
                self.format = Some(match self.format {
                    None => format,
                    Some(previous) if previous == format => format,
                    Some(_) => None,
                });
                if !self.too_many_values {
                    self.values.insert(s.clone());
                    if self.values.len() > ENUM_MAX {
                        self.too_many_values = true;
                        self.values.clear();
                    }
                }
            }
            Value::Array(items) => {
                self.arrays += 1;
                let shape = self.items.get_or_insert_with(Box::default);
                for item in items {
                    shape.add(item);
                }
            }
            Value::Object(map) => {
                self.objects += 1;
                for (key, item) in map {
                    match self.properties.iter_mut().find(|(k, ..)| k == key) {
                        Some((_, count, shape)) => {
                            *count += 1;
                            shape.add(item);
                        }
                        None => {
                            let mut shape = Shape::default();
                            shape.add(item);
                            self.properties.push((key.clone(), 1, shape));
                        }
                    }
                }
            }
        }
    }

    /// 每种出现过的类型各自的 Schema（不含 null 以外的组合）
    fn branches(&self, enums: bool) -> Vec<Value> {
        let mut out = Vec::new();
        if self.objects > 0 {
            let mut properties = Map::new();
            let mut required = Vec::new();
            for (key, count, shape) in &self.properties {
                properties.insert(key.clone(), shape.schema(enums));
                if *count == self.objects {
                    required.push(Value::String(key.clone()));
                }
            }
            let mut schema = json!({"type": "object"});
            if !properties.is_empty() {
                schema["properties"] = Value::Object(properties);
            }
            if !required.is_empty() {
                schema["required"] = Value::Array(required);
            }
            out.push(schema);
        }
        if self.arrays > 0 {
            let mut schema = json!({"type": "array"});
            if let Some(items) = self.items.as_deref().filter(|s| !s.is_empty()) {
                schema["items"] = items.schema(enums);
            }
            out.push(schema);
        }
        if self.strings > 0 {
            let mut schema = json!({"type": "string"});
            if let Some(Some(format)) = self.format {
                schema["format"] = json!(format.name());
            } else if enums
                && !self.too_many_values
                && self.strings >= ENUM_MIN_OCCURRENCES
                && self.values.len() < self.strings
            {
                schema["enum"] = self.values.iter().cloned().collect();
            }
            out.push(schema);
        }
        // 同时出现整数与小数时合并为 number
        if self.numbers > 0 {
            out.push(json!({"type": "number"}));
        } else if self.integers > 0 {
            out.push(json!({"type": "integer"}));
        }
        if self.booleans > 0 {
            out.push(json!({"type": "boolean"}));
        }
        out
    }

    fn is_empty(&self) -> bool {
        self.nulls
            + self.booleans
            + self.integers
            + self.numbers
            + self.strings
            + self.arrays
            + self.objects
            == 0
    }

    fn schema(&self, enums: bool) -> Value {
        let mut branches = self.branches(enums);
        if branches.is_empty() {
            return if self.nulls > 0 {
                json!({"type": "null"})
            } else {
                json!({})
            };
        }
        let simple = branches
            .iter()
            .all(|b| b.as_object().is_some_and(|m| m.len() == 1));
        if simple {
            // 只有类型约束时合并为 type 数组，如 ["string", "null"]
            let mut types: Vec<Value> = branches.iter().map(|b| b["type"].clone()).collect();
            if self.nulls > 0 {
                types.push(json!("null"));
            }
            return if types.len() == 1 {
                json!({"type": types.remove(0)})
            } else {
                json!({"type": types})
            };
        }
        if self.nulls > 0 {
            if branches.len() == 1 {
                // 唯一的复杂分支允许 null：type 改为数组即可
                let ty = branches[0]["type"].clone();
                branches[0]["type"] = json!([ty, "null"]);
                if let Some(values) = branches[0].get_mut("enum").and_then(Value::as_array_mut) {
                    values.push(Value::Null);
                }
                return branches.remove(0);
            }
            branches.push(json!({"type": "null"}));
        }
        if branches.len() == 1 {
            branches.remove(0)
        } else {
            json!({"anyOf": branches})
        }
    }
}

fn syntax(err: &serde_json::Error, line_offset: usize) -> PluginError {
    PluginError::new("schema.infer_syntax")
        .with("line", (err.line() + line_offset) as u64)
        .with("column", err.column() as u64)
        .with("detail", err.to_string())
}

fn samples(input: &str, split: Split) -> PluginResult<Vec<Value>> {
    let values = match split {
        Split::Document => vec![serde_json::from_str(input).map_err(|e| syntax(&e, 0))?],
        Split::Array => match serde_json::from_str(input).map_err(|e| syntax(&e, 0))? {
            Value::Array(items) => items,
            _ => return Err(PluginError::new("schema.infer_not_array")),
        },
        Split::Lines => {
            let mut values = Vec::new();
            for (index, line) in input.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                values.push(serde_json::from_str(line).map_err(|e| syntax(&e, index))?);
            }
            values
        }
    };
    if values.is_empty() {
        return Err(PluginError::new("schema.infer_empty"));
    }
    if values.len() > MAX_SAMPLES {
        return Err(PluginError::new("schema.infer_too_many").with("limit", MAX_SAMPLES as u64));
    }
    Ok(values)
}

pub fn run(args: Args) -> PluginResult<Output> {
    if args.input.trim().is_empty() {
        return Err(PluginError::new("schema.infer_empty"));
    }
    guard::text(&args.input, "instance")?;
    let values = samples(&args.input, args.split)?;
    let mut shape = Shape::default();
    for value in &values {
        guard::nodes(value, "instance")?;
        shape.add(value);
    }
    let mut schema = Map::new();
    schema.insert(
        "$schema".into(),
        json!(match args.draft {
            Draft::Draft202012 => "https://json-schema.org/draft/2020-12/schema",
            Draft::Draft7 => "http://json-schema.org/draft-07/schema#",
        }),
    );
    if let Value::Object(body) = shape.schema(args.enums) {
        schema.extend(body);
    }
    let text = serde_json::to_string_pretty(&Value::Object(schema))
        .map_err(|e| PluginError::new("schema.infer_syntax").with("detail", e.to_string()))?;
    Ok(Output {
        schema: text,
        samples: values.len(),
    })
}

#[cfg(test)]
#[path = "infer_test.rs"]
mod tests;
