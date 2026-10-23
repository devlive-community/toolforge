//! 多环境 .env 对比与合并预览。
//!
//! 规则（第一个文件为基准）：
//! - 缺失：基准中有、该文件中没有的键
//! - 额外：基准中没有、该文件中有的键
//! - 空值：键存在但值为空字符串（引号内的空白不算空）
//! - 重复：同一文件中出现多次的键；比较取值时以最后一次出现为准
//! - 不同：两边都有该键但值不同；结果中不返回任何值，避免泄露密钥
//!
//! 只做文本解析，不执行 shell 展开（$VAR、$(cmd) 等按字面比较）。

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::document::Line;

/// 单个文件中某个键的状态
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CellState {
    Missing,
    Empty,
    Set,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub state: CellState,
    pub duplicate: bool,
    /// 与基准值是否相同；基准自身、任一方缺失时为 None
    pub same_as_base: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub key: String,
    pub cells: Vec<Cell>,
    /// 这一行涉及的问题：missing / extra / empty / duplicate / differs
    pub issues: Vec<&'static str>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub entries: usize,
    pub missing: usize,
    pub extra: usize,
    pub empty: usize,
    pub duplicate: usize,
    pub differs: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub summaries: Vec<Summary>,
    pub rows: Vec<Row>,
}

/// 一个文件的键索引：首次出现顺序、出现次数、最后一次的值
struct Index<'a> {
    order: Vec<&'a str>,
    count: HashMap<&'a str, usize>,
    value: HashMap<&'a str, &'a str>,
}

fn index(lines: &[Line]) -> Index<'_> {
    let mut idx = Index {
        order: Vec::new(),
        count: HashMap::new(),
        value: HashMap::new(),
    };
    for line in lines {
        if let Line::Entry { key, value, .. } = line {
            let n = idx.count.entry(key.as_str()).or_insert(0);
            if *n == 0 {
                idx.order.push(key.as_str());
            }
            *n += 1;
            idx.value.insert(key.as_str(), value.as_str());
        }
    }
    idx
}

/// 对比多个已解析的文件；files[0] 为基准
pub fn compare(files: &[Vec<Line>]) -> Comparison {
    let indexes: Vec<Index> = files.iter().map(|lines| index(lines)).collect();
    let Some(base) = indexes.first() else {
        return Comparison {
            summaries: Vec::new(),
            rows: Vec::new(),
        };
    };

    // 键顺序：先基准顺序，再按文件顺序追加其它文件独有的键
    let mut keys: Vec<&str> = base.order.clone();
    let mut seen: HashSet<&str> = keys.iter().copied().collect();
    for idx in &indexes[1..] {
        for key in &idx.order {
            if seen.insert(key) {
                keys.push(key);
            }
        }
    }

    let mut summaries: Vec<Summary> = indexes
        .iter()
        .map(|idx| Summary {
            entries: idx.order.len(),
            ..Summary::default()
        })
        .collect();

    let rows = keys
        .into_iter()
        .map(|key| {
            let base_value = base.value.get(key).copied();
            let mut issues = Vec::new();
            let mut push = |issue| {
                if !issues.contains(&issue) {
                    issues.push(issue);
                }
            };
            let cells = indexes
                .iter()
                .enumerate()
                .map(|(i, idx)| {
                    let summary = &mut summaries[i];
                    let Some(value) = idx.value.get(key).copied() else {
                        // 基准缺的键对其它文件来说是额外键，基准自身不计缺失
                        if i > 0 {
                            summary.missing += 1;
                            push("missing");
                        }
                        return Cell {
                            state: CellState::Missing,
                            duplicate: false,
                            same_as_base: None,
                        };
                    };
                    let duplicate = idx.count.get(key).copied().unwrap_or(0) > 1;
                    if duplicate {
                        summary.duplicate += 1;
                        push("duplicate");
                    }
                    let state = if value.is_empty() {
                        summary.empty += 1;
                        push("empty");
                        CellState::Empty
                    } else {
                        CellState::Set
                    };
                    let same_as_base = match base_value {
                        Some(base_value) if i > 0 => {
                            let same = base_value == value;
                            if !same {
                                summary.differs += 1;
                                push("differs");
                            }
                            Some(same)
                        }
                        Some(_) => None,
                        None => {
                            summary.extra += 1;
                            push("extra");
                            None
                        }
                    };
                    Cell {
                        state,
                        duplicate,
                        same_as_base,
                    }
                })
                .collect();
            Row {
                key: key.to_owned(),
                cells,
                issues,
            }
        })
        .collect();

    Comparison { summaries, rows }
}

/// 基准中某个键前面紧挨着的注释行（中间没有空行），合并时一并带过去
fn leading_comments(lines: &[Line], at: usize) -> Vec<Line> {
    let start = lines[..at]
        .iter()
        .rposition(|line| !matches!(line, Line::Comment { .. }))
        .map_or(0, |i| i + 1);
    lines[start..at].to_vec()
}

/// 把基准中目标文件缺失的键合并进目标文件。
///
/// 目标文件原有的行一律不动；每个缺失键插到「基准中排在它前面、目标中已有的最近一个键」之后，
/// 找不到时追加到末尾。键上方紧邻的注释一起带过去，export 前缀、引号风格与行内注释保持基准的写法；
/// fill 为假时值留空（与 .env.example 一致），为真时复制基准值。返回合并后的行与新增的键。
pub fn merge(base: &[Line], target: &[Line], fill: bool) -> (Vec<Line>, Vec<String>) {
    // 目标中每个键最后一次出现的行号，作为插入锚点
    let mut position: HashMap<&str, usize> = HashMap::new();
    for (i, line) in target.iter().enumerate() {
        if let Line::Entry { key, .. } = line {
            position.insert(key.as_str(), i);
        }
    }

    // 锚点行号 -> 插入的行；None 表示追加到末尾
    let mut after: HashMap<usize, Vec<Line>> = HashMap::new();
    let mut tail: Vec<Line> = Vec::new();
    let mut anchor: Option<usize> = None;
    let mut added: Vec<String> = Vec::new();
    let mut handled: HashSet<&str> = HashSet::new();

    for (i, line) in base.iter().enumerate() {
        let Line::Entry {
            export,
            key,
            value,
            quote,
            inline_comment,
            multiline,
        } = line
        else {
            continue;
        };
        if let Some(&at) = position.get(key.as_str()) {
            anchor = Some(at);
            continue;
        }
        if !handled.insert(key.as_str()) {
            continue;
        }
        let mut block = leading_comments(base, i);
        block.push(Line::Entry {
            export: *export,
            key: key.clone(),
            value: if fill { value.clone() } else { String::new() },
            quote: *quote,
            inline_comment: inline_comment.clone(),
            multiline: fill && *multiline,
        });
        match anchor {
            Some(at) => after.entry(at).or_default().extend(block),
            None => tail.extend(block),
        }
        added.push(key.clone());
    }

    let mut out = Vec::with_capacity(target.len() + added.len() * 2);
    for (i, line) in target.iter().enumerate() {
        out.push(line.clone());
        if let Some(block) = after.remove(&i) {
            out.extend(block);
        }
    }
    if !tail.is_empty() {
        if out.last().is_some_and(|line| *line != Line::Blank) {
            out.push(Line::Blank);
        }
        out.extend(tail);
    }
    (out, added)
}

#[cfg(test)]
#[path = "compare_test.rs"]
mod tests;
