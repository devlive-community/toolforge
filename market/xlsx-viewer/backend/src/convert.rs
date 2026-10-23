//! calamine 单元格转显示文本与类型：日期序列号换算为本地可读文本，
//! 数字去掉二进制浮点的尾巴，混合类型交给列推断。

use std::io::{Read, Seek};

use calamine::{Data, DataRef, Range, XlsxCellReader, XlsxError};
use jiff::civil::date;

use crate::table::{Column, Kind, Table, infer_column};

/// Excel 日期序列号转文本（1900 日期系统，0 = 1899-12-30）
fn date_text(serial: f64) -> String {
    let days = serial.floor() as i64;
    let seconds = ((serial - days as f64) * 86_400.0).round() as i64;
    let base = date(1899, 12, 30);
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

/// Excel 时长序列号转文本：小时可以超过 24
fn duration_text(serial: f64) -> String {
    let seconds = (serial * 86_400.0).round() as i64;
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    format!("{h}:{m:02}:{s:02}")
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

/// 单元格的显示文本与它自己的类型
pub fn cell(data: &Data) -> (String, Kind) {
    match data {
        Data::Empty => (String::new(), Kind::Empty),
        Data::String(s) => (s.clone(), Kind::Text),
        Data::Int(i) => (i.to_string(), Kind::Integer),
        // xlsx 中所有数字都是浮点存储；整数值的浮点归为整数，与表格展示对齐
        Data::Float(f) => {
            let kind = if f.fract() == 0.0 && f.abs() < 1e15 {
                Kind::Integer
            } else {
                Kind::Float
            };
            (number(*f), kind)
        }
        Data::Bool(b) => (if *b { "TRUE" } else { "FALSE" }.to_owned(), Kind::Boolean),
        Data::DateTime(d) if d.is_datetime() => (date_text(d.as_f64()), Kind::Date),
        Data::DateTime(d) => (duration_text(d.as_f64()), Kind::Text),
        Data::DateTimeIso(s) => (s.clone(), Kind::Date),
        Data::DurationIso(s) => (s.clone(), Kind::Text),
        Data::Error(e) => (format!("#{e:?}"), Kind::Text),
    }
}

/// 把一个工作表转为紧凑表格：右侧全空列截断，空行保留（行号仍有意义），
/// 表头行不计入列类型推断，避免文本表头把数字列污染成文本。
pub fn build_table(range: &Range<Data>, header: bool) -> Table {
    let mut used = 0usize;
    for row in range.rows() {
        for (c, data) in row.iter().enumerate() {
            if !matches!(data, Data::Empty) {
                used = used.max(c + 1);
            }
        }
    }
    let mut table = Table::new();
    if used == 0 {
        return table;
    }
    let mut kinds: Vec<Vec<Kind>> = vec![Vec::new(); used];
    let mut texts: Vec<String> = vec![String::new(); used];
    for (r, row) in range.rows().enumerate() {
        for c in 0..used {
            let (text, kind) = cell(&row[c]);
            if !(header && r == 0) {
                kinds[c].push(kind);
            }
            texts[c] = text;
        }
        table.push_row(texts.iter().map(String::as_str));
    }
    let names = header.then(|| table.take_first_row());
    table.columns = (0..used)
        .map(|c| {
            let name = names
                .as_ref()
                .and_then(|n| n.get(c))
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty());
            Column {
                name,
                kind: infer_column(kinds[c].drain(..)),
            }
        })
        .collect();
    table
}

/// xlsx 逐个读取单元格直接写入紧凑表格，不构建整张 Range：大工作表的内存主要花在 Range 上。
/// 与 calamine 构建 Range 的规则一致（跳过空值单元格，边界取自非空单元格），结果与 build_table 相同；
/// 单元格没有按行列顺序出现时返回 None，由调用方退回 build_table。
pub fn stream_table<RS: Read + Seek>(
    cells: &mut XlsxCellReader<'_, RS>,
    header: bool,
    is_cancelled: impl Fn() -> bool,
) -> Result<Option<Table>, StreamError> {
    let mut table = Table::new();
    // 按绝对列号记录的类型，表头行不计入
    let mut kinds: Vec<Vec<Kind>> = Vec::new();
    let mut row_cells: Vec<String> = Vec::new();
    let mut first_row: Option<u32> = None;
    let mut current = 0u32;
    let mut min_col = u32::MAX;
    let mut used = 0usize;
    let mut count = 0u64;
    while let Some(cell) = cells.next_cell()? {
        if matches!(cell.get_value(), DataRef::Empty) {
            continue;
        }
        count += 1;
        if count.is_multiple_of(100_000) && is_cancelled() {
            return Err(StreamError::Cancelled);
        }
        let (row, col) = cell.get_position();
        match first_row {
            None => {
                first_row = Some(row);
                current = row;
            }
            Some(_) if row < current => return Ok(None),
            Some(_) if row > current => {
                table.push_row(row_cells.iter().map(String::as_str));
                row_cells.clear();
                for _ in current + 1..row {
                    table.push_row(std::iter::empty());
                }
                current = row;
            }
            Some(_) => {}
        }
        let col_index = col as usize;
        if col_index < row_cells.len() {
            return Ok(None);
        }
        let (text, kind) = self::cell(&Data::from(cell.get_value().clone()));
        row_cells.resize(col_index, String::new());
        row_cells.push(text);
        min_col = min_col.min(col);
        used = used.max(col_index + 1);
        if !(header && Some(row) == first_row) {
            if kinds.len() <= col_index {
                kinds.resize_with(col_index + 1, Vec::new);
            }
            kinds[col_index].push(kind);
        }
    }
    if first_row.is_none() {
        return Ok(Some(Table::new()));
    }
    table.push_row(row_cells.iter().map(String::as_str));
    let min_col = min_col as usize;
    if min_col > 0 {
        // 左侧整列为空时去掉，与 Range 从第一个非空列开始一致
        let mut shifted = Table::new();
        for row in 0..table.rows() {
            shifted
                .push_row((min_col..table.row_len(row).max(min_col)).map(|c| table.cell(row, c)));
        }
        table = shifted;
    }
    let names = header.then(|| table.take_first_row());
    kinds.resize_with(used, Vec::new);
    table.columns = kinds
        .drain(min_col..used)
        .enumerate()
        .map(|(c, kinds)| Column {
            name: names
                .as_ref()
                .and_then(|n| n.get(c))
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
            kind: infer_column(kinds),
        })
        .collect();
    Ok(Some(table))
}

#[derive(Debug)]
pub enum StreamError {
    Read(XlsxError),
    Cancelled,
}

impl From<XlsxError> for StreamError {
    fn from(error: XlsxError) -> Self {
        StreamError::Read(error)
    }
}

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
