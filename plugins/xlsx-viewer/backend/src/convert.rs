//! calamine 单元格转显示文本与类型：日期序列号换算为本地可读文本，
//! 数字去掉二进制浮点的尾巴，混合类型交给列推断。

use calamine::{Data, Range};
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

#[cfg(test)]
#[path = "convert_test.rs"]
mod tests;
