//! Word → Excel：文档中的每个表格写成一个工作表；没有表格时把段落逐行写入。

use std::path::Path;

use docx_rs::{DocumentChild, Paragraph, TableCellContent, TableChild, TableRowChild, read_docx};
use rust_xlsxwriter::{Format, Workbook};
use tf_plugin_api::{PluginError, PluginResult};

pub enum Content {
    Tables(Vec<Vec<Vec<String>>>),
    Paragraphs(Vec<String>),
}

fn paragraph_text(p: &Paragraph) -> String {
    p.raw_text()
}

pub fn read(path: &Path) -> PluginResult<Content> {
    let bytes = std::fs::read(path)
        .map_err(|e| PluginError::new("doc.read_failed").with("detail", e.to_string()))?;
    let doc = read_docx(&bytes)
        .map_err(|e| PluginError::new("doc.read_failed").with("detail", e.to_string()))?;
    let mut tables = Vec::new();
    let mut paragraphs = Vec::new();
    for child in &doc.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                let text = paragraph_text(p);
                if !text.trim().is_empty() {
                    paragraphs.push(text);
                }
            }
            DocumentChild::Table(table) => {
                let rows: Vec<Vec<String>> = table
                    .rows
                    .iter()
                    .map(|row| {
                        let TableChild::TableRow(row) = row;
                        row.cells
                            .iter()
                            .map(|cell| {
                                let TableRowChild::TableCell(cell) = cell;
                                cell.children
                                    .iter()
                                    .filter_map(|c| match c {
                                        TableCellContent::Paragraph(p) => Some(paragraph_text(p)),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .collect()
                    })
                    .collect();
                if rows.iter().flatten().any(|c| !c.trim().is_empty()) {
                    tables.push(rows);
                }
            }
            _ => {}
        }
    }
    Ok(if tables.is_empty() {
        Content::Paragraphs(paragraphs)
    } else {
        Content::Tables(tables)
    })
}

/// 只有数字（可带负号与一个小数点、无前导 0）才写成数值，编号、电话等保持文本
fn as_number(text: &str) -> Option<f64> {
    let t = text.trim();
    let digits = t.strip_prefix('-').unwrap_or(t);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.") {
        return None;
    }
    // 超过 15 位有效数字会丢精度（如身份证号），按文本写
    if digits.chars().filter(char::is_ascii_digit).count() > 15 {
        return None;
    }
    t.parse().ok()
}

/// Excel 工作表名称：最长 31 个字符，不能含 []:*?/\
fn sheet_name(name: &str) -> String {
    name.chars()
        .filter(|c| !"[]:*?/\\".contains(*c))
        .take(31)
        .collect()
}

pub struct Options {
    pub header_row: bool,
    /// 表格工作表的名称前缀（如“表格”），后面加序号
    pub table_name: String,
    /// 没有表格时段落所在工作表的名称
    pub text_name: String,
}

/// 写成 xlsx，返回 (工作表数, 是否只有段落)
pub fn to_excel(path: &Path, output: &Path, options: &Options) -> PluginResult<(usize, bool)> {
    let content = read(path)?;
    let mut workbook = Workbook::new();
    let bold = Format::new().set_bold();
    let wrap = Format::new().set_text_wrap();
    let write_err = |e: rust_xlsxwriter::XlsxError| {
        PluginError::new("doc.write_failed").with("detail", e.to_string())
    };
    let (count, text_only) = match content {
        Content::Tables(tables) => {
            for (i, rows) in tables.iter().enumerate() {
                let sheet = workbook.add_worksheet();
                sheet
                    .set_name(sheet_name(&format!("{} {}", options.table_name, i + 1)))
                    .map_err(write_err)?;
                for (r, row) in rows.iter().enumerate() {
                    for (c, text) in row.iter().enumerate() {
                        let (r16, c16) = (r as u32, c as u16);
                        let header = options.header_row && r == 0;
                        match (as_number(text), header) {
                            (Some(n), false) => {
                                sheet.write_number(r16, c16, n).map_err(write_err)?;
                            }
                            (_, true) => {
                                sheet
                                    .write_string_with_format(r16, c16, text, &bold)
                                    .map_err(write_err)?;
                            }
                            (None, false) if text.contains('\n') => {
                                sheet
                                    .write_string_with_format(r16, c16, text, &wrap)
                                    .map_err(write_err)?;
                            }
                            (None, false) => {
                                sheet.write_string(r16, c16, text).map_err(write_err)?;
                            }
                        }
                    }
                }
                sheet.autofit();
            }
            (tables.len(), false)
        }
        Content::Paragraphs(paragraphs) => {
            if paragraphs.is_empty() {
                return Err(PluginError::new("doc.empty_document"));
            }
            let sheet = workbook.add_worksheet();
            sheet
                .set_name(sheet_name(&options.text_name))
                .map_err(write_err)?;
            for (r, text) in paragraphs.iter().enumerate() {
                sheet.write_string(r as u32, 0, text).map_err(write_err)?;
            }
            sheet.set_column_width(0, 80).map_err(write_err)?;
            (1, true)
        }
    };
    workbook.save(output).map_err(write_err)?;
    Ok((count, text_only))
}

#[cfg(test)]
#[path = "word_test.rs"]
mod tests;
