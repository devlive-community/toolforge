//! 读取与解析：识别编码（UTF-8 / UTF-16 BOM / GB18030）、分隔符与表头，再解析为紧凑表格。

use std::collections::HashSet;

use encoding_rs::{Encoding, GB18030, UTF_16BE, UTF_16LE};
use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult, cancelled};

use crate::table::{Column, Kind, Table, infer, parse_number};

const DELIMITERS: [u8; 4] = *b",\t;|";

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// 为空时自动识别
    #[serde(default)]
    pub delimiter: Option<String>,
    #[serde(default)]
    pub header: Option<bool>,
    #[serde(default)]
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub delimiter: String,
    pub encoding: String,
    pub has_header: bool,
}

/// 解码为 UTF-8 文本，返回（文本，编码名）
pub fn decode(bytes: Vec<u8>, encoding: Option<&str>) -> PluginResult<(String, String)> {
    let explicit = encoding
        .filter(|e| !e.is_empty() && !e.eq_ignore_ascii_case("auto"))
        .map(|label| {
            Encoding::for_label(label.as_bytes())
                .ok_or_else(|| PluginError::new("csv.unknown_encoding").with("encoding", label))
        })
        .transpose()?;
    let encoding = match explicit {
        Some(encoding) => encoding,
        None if bytes.starts_with(&[0xFF, 0xFE]) => UTF_16LE,
        None if bytes.starts_with(&[0xFE, 0xFF]) => UTF_16BE,
        None => match String::from_utf8(bytes) {
            Ok(mut text) => {
                if text.starts_with('\u{feff}') {
                    text.drain(..3);
                }
                return Ok((text, "UTF-8".into()));
            }
            // Excel 在中文系统上默认导出 GBK / GB18030
            Err(err) => return decode_with(GB18030, &err.into_bytes()),
        },
    };
    decode_with(encoding, &bytes)
}

fn decode_with(encoding: &'static Encoding, bytes: &[u8]) -> PluginResult<(String, String)> {
    let (text, used, _) = encoding.decode(bytes);
    Ok((text.into_owned(), used.name().to_owned()))
}

/// 在开头几十行里找出能让每行字段数最一致（且多于一个字段）的分隔符
pub fn detect_delimiter(text: &str) -> u8 {
    let mut end = text.len().min(64 * 1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let sample = &text[..end];
    let mut best = (0usize, b',');
    for delimiter in DELIMITERS {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(sample.as_bytes());
        let counts: Vec<usize> = reader
            .byte_records()
            .take(50)
            .filter_map(Result::ok)
            .map(|r| r.len())
            .collect();
        let Some(&mode) = counts
            .iter()
            .max_by_key(|n| counts.iter().filter(|m| m == n).count())
        else {
            continue;
        };
        if mode < 2 {
            continue;
        }
        let consistent = counts.iter().filter(|&&n| n == mode).count();
        let score = consistent * 1000 + mode;
        if score > best.0 {
            best = (score, delimiter);
        }
    }
    best.1
}

pub fn parse_delimiter(value: Option<&str>) -> PluginResult<Option<u8>> {
    match value {
        None | Some("") | Some("auto") => Ok(None),
        Some("\\t" | "tab") => Ok(Some(b'\t')),
        Some(v) if v.len() == 1 && v.is_ascii() => Ok(Some(v.as_bytes()[0])),
        Some(v) => Err(PluginError::new("csv.invalid_delimiter").with("delimiter", v)),
    }
}

/// 解析为表格；progress 以已处理的字节数回调
pub fn parse(
    text: &str,
    delimiter: u8,
    mut progress: impl FnMut(u64),
    is_cancelled: impl Fn() -> bool,
) -> PluginResult<Table> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut table = Table::new();
    let mut record = csv::ByteRecord::new();
    let mut count = 0u64;
    loop {
        match reader.read_byte_record(&mut record) {
            Ok(false) => break,
            Ok(true) => {}
            Err(err) => {
                return Err(PluginError::new("csv.parse_failed").with("detail", err.to_string()));
            }
        }
        // 分隔符与引号都是 ASCII，UTF-8 文本切分后的字段仍是合法 UTF-8
        let cells = record
            .iter()
            .map(|field| std::str::from_utf8(field).unwrap_or_default());
        table.push_row(cells);
        count += 1;
        if count.is_multiple_of(10_000) {
            if is_cancelled() {
                return Err(cancelled());
            }
            progress(reader.position().byte());
        }
        if table.rows() >= u32::MAX as usize / 2 {
            return Err(PluginError::new("csv.too_many_rows"));
        }
    }
    progress(text.len() as u64);
    Ok(table)
}

/// 第一行全部非空且都不是数字或日期，并且名称互不相同或下方有带类型（数字、日期等）的列时，视为表头
pub fn looks_like_header(table: &Table) -> bool {
    if table.rows() < 2 {
        return false;
    }
    let width = table.row_len(0);
    let labels = (0..width).all(|c| {
        let cell = table.cell(0, c).trim();
        !cell.is_empty()
            && parse_number(cell).is_none()
            && infer(std::iter::once(cell)) != Kind::Date
    });
    if !labels {
        return false;
    }
    let mut seen = HashSet::new();
    let unique = (0..width).all(|c| seen.insert(table.cell(0, c).trim()));
    unique
        || (0..width).any(|c| {
            !matches!(
                infer((1..table.rows()).map(|r| table.cell(r, c))),
                Kind::Text | Kind::Empty
            )
        })
}

/// 确定表头并推断列类型
pub fn finish(table: &mut Table, header: Option<bool>) -> bool {
    let has_header = header.unwrap_or_else(|| looks_like_header(table));
    let names = if has_header {
        table.take_first_row()
    } else {
        Vec::new()
    };
    let width = (0..table.rows())
        .map(|r| table.row_len(r))
        .max()
        .unwrap_or(0)
        .max(names.len());
    let kinds: Vec<Kind> = (0..width)
        .map(|c| infer((0..table.rows()).map(|r| table.cell(r, c))))
        .collect();
    table.columns = kinds
        .into_iter()
        .enumerate()
        .map(|(c, kind)| Column {
            name: names.get(c).cloned().filter(|n| !n.is_empty()),
            kind,
        })
        .collect();
    has_header
}

pub fn delimiter_name(delimiter: u8) -> String {
    if delimiter == b'\t' {
        "\\t".into()
    } else {
        (delimiter as char).to_string()
    }
}

#[cfg(test)]
#[path = "load_test.rs"]
mod tests;
