//! 紧凑的表格存储：所有单元格文本拼接在一个字符串里，另存每个单元格的结束位置，
//! 内存占用约等于文件大小，而不是每个单元格一次分配。

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Empty,
    Integer,
    Float,
    Boolean,
    Date,
    Text,
}

impl Kind {
    pub fn is_numeric(self) -> bool {
        matches!(self, Kind::Integer | Kind::Float)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    /// 没有表头时为空，界面显示「第 n 列」
    pub name: Option<String>,
    pub kind: Kind,
}

#[derive(Debug, Default)]
pub struct Table {
    text: String,
    /// 每个单元格在 text 中的结束位置
    ends: Vec<u32>,
    /// 每行第一个单元格在 ends 中的下标，最后多一个哨兵
    starts: Vec<u32>,
    pub columns: Vec<Column>,
}

impl Table {
    pub fn new() -> Self {
        Self {
            starts: vec![0],
            ..Default::default()
        }
    }

    pub fn push_row<'a>(&mut self, cells: impl IntoIterator<Item = &'a str>) {
        for cell in cells {
            self.text.push_str(cell);
            self.ends.push(self.text.len() as u32);
        }
        self.starts.push(self.ends.len() as u32);
    }

    pub fn rows(&self) -> usize {
        self.starts.len() - 1
    }

    pub fn width(&self) -> usize {
        self.columns.len()
    }

    /// 行中缺失的单元格视为空字符串
    pub fn cell(&self, row: usize, column: usize) -> &str {
        let start = self.starts[row] as usize;
        let index = start + column;
        if index >= self.starts[row + 1] as usize {
            return "";
        }
        let from = if index == 0 {
            0
        } else {
            self.ends[index - 1] as usize
        };
        &self.text[from..self.ends[index] as usize]
    }

    pub fn row_len(&self, row: usize) -> usize {
        (self.starts[row + 1] - self.starts[row]) as usize
    }

    /// 删除第一行并返回其内容（用作表头）
    pub fn take_first_row(&mut self) -> Vec<String> {
        if self.rows() == 0 {
            return Vec::new();
        }
        let cells: Vec<String> = (0..self.row_len(0))
            .map(|c| self.cell(0, c).to_owned())
            .collect();
        let count = self.starts[1] as usize;
        let bytes = if count == 0 {
            0
        } else {
            self.ends[count - 1] as usize
        };
        self.text.drain(..bytes);
        self.ends.drain(..count);
        for end in &mut self.ends {
            *end -= bytes as u32;
        }
        self.starts.remove(0);
        for start in &mut self.starts {
            *start -= count as u32;
        }
        cells
    }
}

fn is_number(text: &str) -> bool {
    let t = text.as_bytes();
    matches!(t.first(), Some(b'0'..=b'9' | b'-' | b'+' | b'.'))
        && t.iter().any(u8::is_ascii_digit)
        && text.parse::<f64>().is_ok_and(f64::is_finite)
}

pub fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    is_number(text).then(|| text.parse().ok()).flatten()
}

fn is_integer(text: &str) -> bool {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    !digits.is_empty() && digits.len() <= 18 && digits.bytes().all(|b| b.is_ascii_digit())
}

fn is_boolean(text: &str) -> bool {
    ["true", "false", "yes", "no"]
        .iter()
        .any(|v| text.eq_ignore_ascii_case(v))
}

/// yyyy-mm-dd 或 yyyy/mm/dd，后面可以跟时间
fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && matches!(b[4], b'-' | b'/')
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b[4]
        && b[8..10].iter().all(u8::is_ascii_digit)
        && (b.len() == 10 || matches!(b[10], b' ' | b'T'))
}

/// 根据所有非空值推断列类型
pub fn infer(values: impl Iterator<Item = impl AsRef<str>>) -> Kind {
    let (mut int, mut float, mut boolean, mut date, mut any) = (true, true, true, true, false);
    for value in values {
        let v = value.as_ref().trim();
        if v.is_empty() {
            continue;
        }
        any = true;
        int = int && is_integer(v);
        float = float && is_number(v);
        boolean = boolean && is_boolean(v);
        date = date && is_date(v);
        if !(int || float || boolean || date) {
            return Kind::Text;
        }
    }
    match () {
        _ if !any => Kind::Empty,
        _ if int => Kind::Integer,
        _ if float => Kind::Float,
        _ if boolean => Kind::Boolean,
        _ if date => Kind::Date,
        _ => Kind::Text,
    }
}

#[cfg(test)]
#[path = "table_test.rs"]
mod tests;
