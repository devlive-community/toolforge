//! 表格的统一表示与通用变换。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    #[default]
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Table {
    /// 表头；没有表头时为空
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// 每列的对齐方式（来自 Markdown 分隔行或用户设置）
    pub aligns: Vec<Align>,
}

impl Table {
    pub fn columns(&self) -> usize {
        self.rows
            .iter()
            .map(Vec::len)
            .chain(std::iter::once(self.headers.len()))
            .max()
            .unwrap_or(0)
    }

    /// 补齐每行的列数，去掉全空的行
    pub fn normalize(mut self) -> Self {
        let n = self.columns();
        if !self.headers.is_empty() {
            self.headers.resize(n, String::new());
        }
        self.rows
            .retain(|row| row.iter().any(|c| !c.trim().is_empty()));
        for row in &mut self.rows {
            row.resize(n, String::new());
        }
        self.aligns.resize(n, Align::None);
        self
    }

    /// 没有表头时把第一行当作表头，或反过来把表头放回第一行
    pub fn with_header(mut self, header: bool) -> Self {
        match (header, self.headers.is_empty()) {
            (true, true) if !self.rows.is_empty() => self.headers = self.rows.remove(0),
            (false, false) => self.rows.insert(0, std::mem::take(&mut self.headers)),
            _ => {}
        }
        self
    }

    pub fn trim(mut self) -> Self {
        for cell in self
            .headers
            .iter_mut()
            .chain(self.rows.iter_mut().flatten())
        {
            *cell = cell.trim().to_owned();
        }
        self
    }

    /// 行列互换（表头作为第一列参与）
    pub fn transpose(self) -> Self {
        let had_header = !self.headers.is_empty();
        let mut all = Vec::with_capacity(self.rows.len() + 1);
        if had_header {
            all.push(self.headers);
        }
        all.extend(self.rows);
        let n = all.iter().map(Vec::len).max().unwrap_or(0);
        let mut rows: Vec<Vec<String>> = (0..n)
            .map(|c| {
                all.iter()
                    .map(|r| r.get(c).cloned().unwrap_or_default())
                    .collect()
            })
            .collect();
        let headers = if had_header && !rows.is_empty() {
            rows.remove(0)
        } else {
            Vec::new()
        };
        Table {
            headers,
            rows,
            aligns: Vec::new(),
        }
        .normalize()
    }
}

#[cfg(test)]
#[path = "table_test.rs"]
mod tests;
