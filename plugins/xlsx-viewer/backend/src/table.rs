//! 紧凑的表格存储：所有单元格文本拼接在一个字符串里，另存每个单元格的结束位置，
//! 内存占用约等于数据本身的文本大小，而不是每个单元格一次分配。

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

pub fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// 根据一列中每个非空单元格的类型推断列类型；混合类型显示为文本
pub fn infer_column(kinds: impl IntoIterator<Item = Kind>) -> Kind {
    let mut result = Kind::Empty;
    for kind in kinds {
        if kind == Kind::Empty {
            continue;
        }
        if result == Kind::Empty || result == kind {
            result = kind;
        } else if result.is_numeric() && kind.is_numeric() {
            result = Kind::Float;
        } else {
            return Kind::Text;
        }
    }
    result
}

#[cfg(test)]
#[path = "table_test.rs"]
mod tests;
