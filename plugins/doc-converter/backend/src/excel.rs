//! Excel → Word：每个非空工作表写成“标题 + 表格”，首行加粗，列多时用横向页面。

use std::path::Path;

use calamine::{Data, Reader, open_workbook_auto};
use docx_rs::{Docx, PageOrientationType, Paragraph, Run, Table, TableCell, TableRow};
use tf_plugin_api::{PluginError, PluginResult};

/// 超过这么多列时改用横向页面
const LANDSCAPE_COLUMNS: usize = 7;
/// A4 尺寸（twip）
const A4: (u32, u32) = (11906, 16838);

/// Excel 日期序列号转文本（1900 日期系统，0 = 1899-12-30）
fn excel_date(serial: f64) -> String {
    let days = serial.floor() as i64;
    let seconds = ((serial - serial.floor()) * 86_400.0).round() as i64;
    let base = jiff::civil::date(1899, 12, 30);
    let date = base
        .checked_add(jiff::Span::new().days(days))
        .unwrap_or(base);
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    match (days == 0, seconds == 0) {
        (true, false) => format!("{h:02}:{m:02}:{s:02}"),
        (_, true) => date.strftime("%Y-%m-%d").to_string(),
        (false, false) => format!("{} {h:02}:{m:02}:{s:02}", date.strftime("%Y-%m-%d")),
    }
}

/// 数字去掉二进制浮点的尾巴（0.1 + 0.2 显示为 0.3）
fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let rounded = format!("{value:.10}");
    rounded
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

pub fn cell_text(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => number(*f),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        Data::DateTime(d) if d.is_datetime() => excel_date(d.as_f64()),
        Data::DateTime(d) => number(d.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#{e:?}"),
    }
}

/// 读取所有工作表：(名称, 去掉空行与右侧空列后的单元格文本)
pub fn read(path: &Path) -> PluginResult<Vec<(String, Vec<Vec<String>>)>> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| PluginError::new("doc.read_failed").with("detail", e.to_string()))?;
    let mut sheets = Vec::new();
    for name in workbook.sheet_names().clone() {
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| PluginError::new("doc.read_failed").with("detail", e.to_string()))?;
        let mut rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(cell_text).collect::<Vec<_>>())
            .filter(|row| row.iter().any(|c| !c.trim().is_empty()))
            .collect();
        let width = rows
            .iter()
            .map(|r| {
                r.iter()
                    .rposition(|c| !c.trim().is_empty())
                    .map_or(0, |i| i + 1)
            })
            .max()
            .unwrap_or(0);
        for row in &mut rows {
            row.truncate(width);
        }
        if !rows.is_empty() {
            sheets.push((name, rows));
        }
    }
    Ok(sheets)
}

fn cell(text: &str, bold: bool) -> TableCell {
    // 单元格中的换行拆成多段
    let mut cell = TableCell::new();
    for line in text.split('\n') {
        let mut run = Run::new().add_text(line);
        if bold {
            run = run.bold();
        }
        cell = cell.add_paragraph(Paragraph::new().add_run(run));
    }
    cell
}

pub struct Options {
    pub header_row: bool,
    pub sheet_titles: bool,
}

/// 写成 docx，返回写入的工作表数
pub fn to_word(path: &Path, output: &Path, options: &Options) -> PluginResult<usize> {
    let sheets = read(path)?;
    if sheets.is_empty() {
        return Err(PluginError::new("doc.empty_workbook"));
    }
    let widest = sheets
        .iter()
        .map(|(_, rows)| rows.iter().map(Vec::len).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let mut docx = Docx::new();
    if widest > LANDSCAPE_COLUMNS {
        docx = docx
            .page_size(A4.1, A4.0)
            .page_orient(PageOrientationType::Landscape);
    }
    for (index, (name, rows)) in sheets.iter().enumerate() {
        if options.sheet_titles {
            let mut title = Paragraph::new().add_run(Run::new().add_text(name).bold().size(28));
            if index > 0 {
                title = title.page_break_before(true);
            }
            docx = docx.add_paragraph(title);
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        let table_rows: Vec<TableRow> = rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                let cells = (0..width)
                    .map(|c| {
                        cell(
                            row.get(c).map_or("", String::as_str),
                            options.header_row && r == 0,
                        )
                    })
                    .collect();
                TableRow::new(cells)
            })
            .collect();
        docx = docx.add_table(Table::new(table_rows));
        docx = docx.add_paragraph(Paragraph::new());
    }
    let file = std::fs::File::create(output)
        .map_err(|e| PluginError::new("doc.write_failed").with("detail", e.to_string()))?;
    docx.build()
        .pack(file)
        .map_err(|e| PluginError::new("doc.write_failed").with("detail", e.to_string()))?;
    Ok(sheets.len())
}

#[cfg(test)]
#[path = "excel_test.rs"]
mod tests;
