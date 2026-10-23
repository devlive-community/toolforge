//! XML 与 JSON 数据模型互转。
//!
//! 约定：属性映射为 `@名称`，元素同时有属性或子元素时文本映射为 `#text`；
//! 同名子元素合并为数组；空元素为 null。注释、处理指令与 DOCTYPE 被忽略。

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use serde_json::{Map, Value};
use tf_plugin_api::{PluginError, PluginResult};

use crate::convert::{Warning, line_col};
use crate::csvfmt::infer;

pub const ATTR_PREFIX: char = '@';
pub const TEXT_KEY: &str = "#text";
const ROOT: &str = "root";
const ITEM: &str = "item";

struct Frame {
    name: String,
    map: Map<String, Value>,
    text: String,
}

fn scalar(text: &str, infer_types: bool) -> Value {
    if infer_types {
        infer(text)
    } else {
        Value::String(text.to_owned())
    }
}

fn insert_child(map: &mut Map<String, Value>, key: String, value: Value) {
    match map.get_mut(&key) {
        Some(Value::Array(items)) if !key.starts_with(ATTR_PREFIX) => items.push(value),
        Some(existing) => {
            let first = existing.take();
            *existing = Value::Array(vec![first, value]);
        }
        None => {
            map.insert(key, value);
        }
    }
}

fn entity(name: &str) -> Option<char> {
    Some(match name {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "apos" => '\'',
        "quot" => '"',
        _ => return None,
    })
}

pub fn parse(input: &str, infer_types: bool) -> PluginResult<Value> {
    let mut reader = Reader::from_str(input);
    let syntax = |reader: &Reader<&[u8]>, detail: String| {
        let (line, column) = line_col(input, reader.error_position() as usize);
        PluginError::new("format.xml_invalid")
            .with("line", line)
            .with("column", column)
            .with("detail", detail)
    };
    let mut stack: Vec<Frame> = Vec::new();
    let mut root: Option<(String, Value)> = None;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| syntax(&reader, e.to_string()))?;
        match event {
            Event::Start(ref tag) | Event::Empty(ref tag) => {
                if root.is_some() && stack.is_empty() {
                    return Err(syntax(&reader, "multiple root elements".into()));
                }
                let mut map = Map::new();
                for attr in tag.attributes() {
                    let attr = attr.map_err(|e| syntax(&reader, e.to_string()))?;
                    let value = attr
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|e| syntax(&reader, e.to_string()))?;
                    map.insert(
                        format!("{ATTR_PREFIX}{}", attr.key.as_ref()),
                        scalar(&value, infer_types),
                    );
                }
                stack.push(Frame {
                    name: tag.name().as_ref().to_owned(),
                    map,
                    text: String::new(),
                });
                if matches!(event, Event::Empty(_)) {
                    close(&mut stack, &mut root, infer_types);
                }
            }
            Event::End(_) => close(&mut stack, &mut root, infer_types),
            Event::Text(text) => {
                if let Some(frame) = stack.last_mut() {
                    frame.text.push_str(&text.xml10_content());
                } else if !text.trim().is_empty() {
                    return Err(syntax(&reader, "text outside the root element".into()));
                }
            }
            Event::CData(data) => {
                if let Some(frame) = stack.last_mut() {
                    frame.text.push_str(&data.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                let ch = match reference
                    .resolve_char_ref()
                    .map_err(|e| syntax(&reader, e.to_string()))?
                {
                    Some(ch) => ch,
                    None => entity(&reference).ok_or_else(|| {
                        syntax(&reader, format!("unknown entity &{};", &*reference))
                    })?,
                };
                if let Some(frame) = stack.last_mut() {
                    frame.text.push(ch);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some(frame) = stack.last() {
        return Err(PluginError::new("format.xml_invalid")
            .with("detail", format!("unclosed element <{}>", frame.name)));
    }
    let (name, value) = root.ok_or_else(|| {
        PluginError::new("format.xml_invalid").with("detail", "no root element".to_owned())
    })?;
    let mut map = Map::new();
    map.insert(name, value);
    Ok(Value::Object(map))
}

fn close(stack: &mut Vec<Frame>, root: &mut Option<(String, Value)>, infer_types: bool) {
    let Some(frame) = stack.pop() else { return };
    let text = frame.text.trim();
    let value = if frame.map.is_empty() {
        if text.is_empty() {
            Value::Null
        } else {
            scalar(text, infer_types)
        }
    } else {
        let mut map = frame.map;
        if !text.is_empty() {
            map.insert(TEXT_KEY.to_owned(), scalar(text, infer_types));
        }
        Value::Object(map)
    };
    match stack.last_mut() {
        Some(parent) => insert_child(&mut parent.map, frame.name, value),
        None => *root = Some((frame.name, value)),
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let start = |c: char| c.is_alphabetic() || c == '_' || c == ':';
    start(first)
        && chars.all(|c| start(c) || c.is_alphanumeric() || c == '-' || c == '.')
        && !name.to_ascii_lowercase().starts_with("xml")
}

fn escape(text: &str, attr: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' if attr => out.push_str("&quot;"),
            '\n' if attr => out.push_str("&#10;"),
            _ => out.push(c),
        }
    }
    out
}

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

struct Writer<'a> {
    out: String,
    indent: &'a str,
    warnings: &'a mut Vec<Warning>,
}

impl Writer<'_> {
    fn newline(&mut self, depth: usize) {
        if !self.indent.is_empty() {
            self.out.push('\n');
            for _ in 0..depth {
                self.out.push_str(self.indent);
            }
        }
    }

    fn element(&mut self, name: &str, value: &Value, depth: usize, path: &str) -> PluginResult<()> {
        if let Value::Array(items) = value {
            if items.len() == 1 {
                self.warnings.push(Warning::new("xml_single_array", path));
            }
            for (index, item) in items.iter().enumerate() {
                if item.is_array() {
                    return Err(PluginError::new("format.xml_nested_array")
                        .with("path", format!("{path}[{index}]")));
                }
                self.element(name, item, depth, &format!("{path}[{index}]"))?;
            }
            return Ok(());
        }
        if !valid_name(name) {
            return Err(PluginError::new("format.xml_name")
                .with("name", name.to_owned())
                .with("path", path.to_owned()));
        }
        if depth > 0 || !self.out.is_empty() {
            self.newline(depth);
        }
        self.out.push('<');
        self.out.push_str(name);
        let Value::Object(map) = value else {
            match text_of(value) {
                Some(text) => {
                    self.out.push('>');
                    self.out.push_str(&escape(&text, false));
                    self.out.push_str(&format!("</{name}>"));
                }
                None => self.out.push_str("/>"),
            }
            return Ok(());
        };

        let mut text = None;
        let mut children = Vec::new();
        for (key, item) in map {
            if let Some(attr) = key.strip_prefix(ATTR_PREFIX) {
                let Some(content) = text_of(item).or_else(|| item.is_null().then(String::new))
                else {
                    return Err(PluginError::new("format.xml_attribute")
                        .with("path", format!("{path}.{key}")));
                };
                if !valid_name(attr) {
                    return Err(PluginError::new("format.xml_name")
                        .with("name", attr.to_owned())
                        .with("path", format!("{path}.{key}")));
                }
                self.out
                    .push_str(&format!(" {attr}=\"{}\"", escape(&content, true)));
            } else if key == TEXT_KEY {
                text = text_of(item);
            } else {
                children.push((key, item));
            }
        }
        if children.is_empty() {
            match text {
                Some(text) => {
                    self.out.push('>');
                    self.out.push_str(&escape(&text, false));
                    self.out.push_str(&format!("</{name}>"));
                }
                None => self.out.push_str("/>"),
            }
            return Ok(());
        }
        self.out.push('>');
        if let Some(text) = text {
            self.newline(depth + 1);
            self.out.push_str(&escape(&text, false));
        }
        for (key, item) in children {
            self.element(key, item, depth + 1, &format!("{path}.{key}"))?;
        }
        self.newline(depth);
        self.out.push_str(&format!("</{name}>"));
        Ok(())
    }
}

/// 生成 XML；indent 为 0 时输出单行
pub fn write(value: &Value, indent: usize, warnings: &mut Vec<Warning>) -> PluginResult<String> {
    let indent = " ".repeat(indent);
    let mut writer = Writer {
        out: String::new(),
        indent: &indent,
        warnings,
    };
    let single_root = match value {
        Value::Object(map) => {
            let mut elements = map.iter().filter(|(k, _)| !k.starts_with(ATTR_PREFIX));
            match (elements.next(), elements.next()) {
                (Some((key, item)), None) if map.len() == 1 && !item.is_array() => {
                    Some((key, item))
                }
                _ => None,
            }
        }
        _ => None,
    };
    match (single_root, value) {
        (Some((key, item)), _) => writer.element(key, item, 0, &format!("$.{key}"))?,
        (None, Value::Array(items)) => {
            let wrapped = Value::Object(Map::from_iter([(
                ITEM.to_owned(),
                Value::Array(items.clone()),
            )]));
            writer.warnings.push(Warning::new("xml_wrapped_root", "$"));
            writer.element(ROOT, &wrapped, 0, "$")?;
        }
        (None, _) => {
            writer.warnings.push(Warning::new("xml_wrapped_root", "$"));
            writer.element(ROOT, value, 0, "$")?;
        }
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n",
        writer.out
    ))
}

#[cfg(test)]
#[path = "xmlfmt_test.rs"]
mod tests;
