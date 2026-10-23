//! 视图：排序、全局搜索与列筛选都在这里计算，前端只按页读取当前视图中的行。

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::table::{Table, parse_number};

/// 单个单元格返回给前端的最大字符数
const MAX_CELL_CHARS: usize = 4096;
pub const MAX_PAGE: usize = 1000;
const TOP_VALUES: usize = 10;
const MAX_DISTINCT: usize = 100_000;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SortKey {
    pub column: usize,
    #[serde(default)]
    pub desc: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Contains,
    Equals,
    NotEquals,
    Greater,
    Less,
    Empty,
    NotEmpty,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Filter {
    pub column: usize,
    pub op: Op,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Spec {
    #[serde(default)]
    pub sort: Vec<SortKey>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub filters: Vec<Filter>,
}

impl Spec {
    pub fn is_identity(&self) -> bool {
        self.sort.is_empty() && self.query.trim().is_empty() && self.filters.is_empty()
    }

    fn check(&self, table: &Table) -> PluginResult<()> {
        let columns = self
            .sort
            .iter()
            .map(|s| s.column)
            .chain(self.filters.iter().map(|f| f.column));
        for column in columns {
            if column >= table.width() {
                return Err(PluginError::new("csv.invalid_column").with("column", column));
            }
        }
        Ok(())
    }
}

/// 大小写不敏感的包含判断；needle 已转为小写
fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.is_ascii() {
        let (h, n) = (hay.as_bytes(), needle.as_bytes());
        return h.len() >= n.len() && h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n));
    }
    hay.to_lowercase().contains(needle)
}

fn compare_values(cell: &str, value: &str) -> Ordering {
    match (parse_number(cell), parse_number(value)) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        _ => cell.cmp(value),
    }
}

fn matches(filter: &Filter, cell: &str, needle: &str) -> bool {
    let trimmed = cell.trim();
    match filter.op {
        Op::Contains => contains_ci(cell, needle),
        Op::Equals => cell == filter.value,
        Op::NotEquals => cell != filter.value,
        Op::Greater => !trimmed.is_empty() && compare_values(trimmed, filter.value.trim()).is_gt(),
        Op::Less => !trimmed.is_empty() && compare_values(trimmed, filter.value.trim()).is_lt(),
        Op::Empty => trimmed.is_empty(),
        Op::NotEmpty => !trimmed.is_empty(),
    }
}

enum Keys {
    Number(Vec<Option<f64>>),
    Text(Vec<Option<String>>),
}

impl Keys {
    fn build(table: &Table, column: usize) -> Self {
        let rows = 0..table.rows();
        if table.columns[column].kind.is_numeric() {
            Keys::Number(rows.map(|r| parse_number(table.cell(r, column))).collect())
        } else {
            Keys::Text(
                rows.map(|r| {
                    let cell = table.cell(r, column).trim();
                    (!cell.is_empty()).then(|| cell.to_lowercase())
                })
                .collect(),
            )
        }
    }

    /// 空值总排在最后，不受升降序影响
    fn compare(&self, a: u32, b: u32, desc: bool) -> Ordering {
        let flip = |o: Ordering| if desc { o.reverse() } else { o };
        let pick = |x: Option<Ordering>, a_some: bool, b_some: bool| match (a_some, b_some) {
            (true, true) => flip(x.unwrap_or(Ordering::Equal)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => Ordering::Equal,
        };
        let (a, b) = (a as usize, b as usize);
        match self {
            Keys::Number(v) => pick(
                v[a].zip(v[b]).map(|(x, y)| x.total_cmp(&y)),
                v[a].is_some(),
                v[b].is_some(),
            ),
            Keys::Text(v) => pick(
                v[a].as_ref().zip(v[b].as_ref()).map(|(x, y)| x.cmp(y)),
                v[a].is_some(),
                v[b].is_some(),
            ),
        }
    }
}

/// 计算视图中的行号；无排序与筛选时返回 None 表示原始顺序
pub fn build(table: &Table, spec: &Spec) -> PluginResult<Option<Vec<u32>>> {
    spec.check(table)?;
    if spec.is_identity() {
        return Ok(None);
    }
    let query = spec.query.trim().to_lowercase();
    let needles: Vec<String> = spec
        .filters
        .iter()
        .map(|f| f.value.to_lowercase())
        .collect();
    let mut rows: Vec<u32> = (0..table.rows() as u32)
        .filter(|&r| {
            let r = r as usize;
            let row_ok = query.is_empty()
                || (0..table.row_len(r)).any(|c| contains_ci(table.cell(r, c), &query));
            row_ok
                && spec
                    .filters
                    .iter()
                    .zip(&needles)
                    .all(|(f, needle)| matches(f, table.cell(r, f.column), needle))
        })
        .collect();
    if !spec.sort.is_empty() {
        let keys: Vec<(Keys, bool)> = spec
            .sort
            .iter()
            .map(|s| (Keys::build(table, s.column), s.desc))
            .collect();
        rows.sort_by(|&a, &b| {
            keys.iter()
                .map(|(k, desc)| k.compare(a, b, *desc))
                .find(|o| o.is_ne())
                .unwrap_or(Ordering::Equal)
        });
    }
    Ok(Some(rows))
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Row {
    /// 原始行号（从 0 开始）
    pub index: u32,
    pub cells: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Page {
    pub total: usize,
    pub offset: usize,
    pub rows: Vec<Row>,
}

fn clip(cell: &str) -> String {
    match cell.char_indices().nth(MAX_CELL_CHARS) {
        Some((end, _)) => format!("{}…", &cell[..end]),
        None => cell.to_owned(),
    }
}

pub fn row_at(view: Option<&[u32]>, position: usize) -> usize {
    view.map_or(position, |v| v[position] as usize)
}

pub fn view_len(table: &Table, view: Option<&[u32]>) -> usize {
    view.map_or(table.rows(), <[u32]>::len)
}

pub fn page(table: &Table, view: Option<&[u32]>, offset: usize, limit: usize) -> Page {
    let total = view_len(table, view);
    let end = offset.saturating_add(limit.min(MAX_PAGE)).min(total);
    let rows = (offset.min(end)..end)
        .map(|position| {
            let row = row_at(view, position);
            Row {
                index: row as u32,
                cells: (0..table.width())
                    .map(|c| clip(table.cell(row, c)))
                    .collect(),
            }
        })
        .collect();
    Page {
        total,
        offset,
        rows,
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ValueCount {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub count: usize,
    pub empty: usize,
    pub distinct: usize,
    /// 不同值超过上限时停止计数
    pub distinct_capped: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub sum: Option<f64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub top: Vec<ValueCount>,
}

pub fn stats(table: &Table, view: Option<&[u32]>, column: usize) -> PluginResult<Stats> {
    if column >= table.width() {
        return Err(PluginError::new("csv.invalid_column").with("column", column));
    }
    let numeric = table.columns[column].kind.is_numeric();
    let total = view_len(table, view);
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut capped = false;
    let (mut empty, mut sum, mut numbers) = (0, 0.0, 0usize);
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_len, mut max_len) = (usize::MAX, 0);
    for position in 0..total {
        let cell = table.cell(row_at(view, position), column);
        if cell.trim().is_empty() {
            empty += 1;
            continue;
        }
        if let Some(count) = counts.get_mut(cell) {
            *count += 1;
        } else if counts.len() < MAX_DISTINCT {
            counts.insert(cell, 1);
        } else {
            capped = true;
        }
        let length = cell.chars().count();
        min_len = min_len.min(length);
        max_len = max_len.max(length);
        if numeric && let Some(n) = parse_number(cell) {
            numbers += 1;
            sum += n;
            min = min.min(n);
            max = max.max(n);
        }
    }
    let mut top: Vec<ValueCount> = counts
        .iter()
        .map(|(value, count)| ValueCount {
            value: clip(value),
            count: *count,
        })
        .collect();
    top.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    top.truncate(TOP_VALUES);
    let has_numbers = numbers > 0;
    Ok(Stats {
        count: total,
        empty,
        distinct: counts.len(),
        distinct_capped: capped,
        min: has_numbers.then_some(min),
        max: has_numbers.then_some(max),
        mean: has_numbers.then(|| sum / numbers as f64),
        sum: has_numbers.then_some(sum),
        min_length: (max_len > 0).then_some(min_len),
        max_length: (max_len > 0).then_some(max_len),
        top,
    })
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
