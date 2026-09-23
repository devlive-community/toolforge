use std::time::Instant;

use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

const MAX_INPUT: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Pretty,
    Minify,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
pub enum Indent {
    #[default]
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "4")]
    Four,
    #[serde(rename = "tab")]
    Tab,
}

#[derive(Deserialize)]
pub struct Args {
    input: String,
    #[serde(default)]
    indent: Indent,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub elements: usize,
    pub attributes: usize,
    pub comments: usize,
    pub max_depth: usize,
    pub lines: usize,
    pub chars: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output: String,
    pub stats: Stats,
    pub elapsed_ms: f64,
}

/// 字节偏移 → 1 起始的行列号（列按字符计）
fn line_col(input: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(input.len());
    let before = &input[..input.floor_char_boundary(pos)];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|l| l.chars().count())
        .unwrap_or(0)
        + 1;
    (line, column)
}

fn located(code: &str, input: &str, pos: u64) -> PluginError {
    let (line, column) = line_col(input, pos as usize);
    PluginError::new(code)
        .with("line", line)
        .with("column", column)
}

pub fn process(args: Args, mode: Mode) -> PluginResult<Output> {
    let input = args.input.as_str();
    if input.trim().is_empty() {
        return Err(PluginError::new("xml.empty"));
    }
    if input.len() > MAX_INPUT {
        return Err(PluginError::new("xml.too_large").with("limit", "20 MB"));
    }
    let start = Instant::now();

    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);
    let mut writer = match (mode, args.indent) {
        (Mode::Minify, _) => Writer::new(Vec::new()),
        (Mode::Pretty, Indent::Two) => Writer::new_with_indent(Vec::new(), b' ', 2),
        (Mode::Pretty, Indent::Four) => Writer::new_with_indent(Vec::new(), b' ', 4),
        (Mode::Pretty, Indent::Tab) => Writer::new_with_indent(Vec::new(), b'\t', 1),
    };

    let mut stats = Stats::default();
    // 未闭合标签栈：quick-xml 在文件结束时不会报告未闭合元素
    let mut open: Vec<(String, u64)> = Vec::new();
    let mut roots = 0;

    loop {
        let position = reader.buffer_position();
        let event = reader.read_event().map_err(|e| {
            located("xml.syntax", input, reader.error_position()).with("detail", e.to_string())
        })?;
        // 读取前的位置可能位于被 trim 掉的空白之前：标签取其 '<' 的真实位置，文本跳过前导空白
        let end = (reader.buffer_position() as usize).min(input.len());
        let position = match &event {
            Event::Start(_) | Event::Empty(_) => input[..end].rfind('<').unwrap_or(0) as u64,
            _ => {
                let from = position as usize;
                let skipped = input[from..end].len() - input[from..end].trim_start().len();
                (from + skipped) as u64
            }
        };
        match &event {
            Event::Start(tag) | Event::Empty(tag) => {
                if open.is_empty() {
                    roots += 1;
                    if roots > 1 {
                        return Err(located("xml.multiple_roots", input, position));
                    }
                }
                stats.elements += 1;
                stats.attributes += tag.attributes().count();
                if matches!(event, Event::Start(_)) {
                    open.push((tag.name().as_ref().to_owned(), position));
                    stats.max_depth = stats.max_depth.max(open.len());
                } else {
                    stats.max_depth = stats.max_depth.max(open.len() + 1);
                }
            }
            Event::End(_) => {
                open.pop();
            }
            Event::Comment(_) => stats.comments += 1,
            Event::Text(text) if open.is_empty() && !text.as_ref().trim().is_empty() => {
                return Err(located("xml.text_outside_root", input, position));
            }
            Event::Eof => break,
            _ => {}
        }
        writer
            .write_event(event)
            .map_err(|e| PluginError::new("xml.serialize_failed").with("detail", e.to_string()))?;
    }

    if let Some((name, position)) = open.last() {
        return Err(located("xml.unclosed", input, *position).with("tag", name.as_str()));
    }
    if roots == 0 {
        return Err(PluginError::new("xml.no_root"));
    }

    let output = String::from_utf8(writer.into_inner())
        .map_err(|e| PluginError::new("xml.serialize_failed").with("detail", e.to_string()))?;
    stats.lines = output.lines().count().max(1);
    stats.chars = output.chars().count();
    Ok(Output {
        output,
        stats,
        elapsed_ms: (start.elapsed().as_secs_f64() * 100_000.0).round() / 100.0,
    })
}

#[cfg(test)]
#[path = "xml_test.rs"]
mod tests;
