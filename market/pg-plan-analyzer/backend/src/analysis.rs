//! 计划分析：构建节点树、按 loops 口径拆分包含/自身耗时、生成估计偏差与
//! 排序落盘等提示，并挑选热点节点。
//!
//! 指标口径：EXPLAIN 的 Actual Total Time 是「每次循环」的平均毫秒，节点对
//! 父节点的贡献 = Actual Total Time × Actual Loops；自身耗时 = 包含耗时 −
//! 子节点包含耗时之和。绝不把整棵树的 Actual Total Time 直接相加。

use serde::Serialize;
use serde_json::Value;
use tf_plugin_api::PluginError;

use crate::plan::{Limits, RawNode, RawStatement};

/// 估计行数与实际行数偏差提示的阈值：倍数与绝对行数都满足才提示
const DEVIATION_FACTOR: f64 = 10.0;
const DEVIATION_MIN_ROWS: f64 = 100.0;
/// 顺序扫描行数提示阈值（只提示，不定性为错误）
const SEQ_SCAN_ROWS: f64 = 10_000.0;
/// 热点数量上限
pub const HOTSPOT_LIMIT: usize = 5;

/// 非致命提示：code 由前端 i18n，params 提供插值
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Diag {
    pub code: String,
    #[serde(default)]
    pub params: Value,
}

impl Diag {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            params: Value::Object(serde_json::Map::new()),
        }
    }

    pub fn param(mut self, key: &str, value: impl Into<Value>) -> Self {
        if let Value::Object(params) = &mut self.params {
            params.insert(key.to_owned(), value.into());
        }
        self
    }
}

#[derive(Debug, Serialize)]
pub struct Node {
    pub id: u32,
    pub depth: u32,
    pub label: String,
    pub node_type: String,
    pub relation: Option<String>,
    pub index_name: Option<String>,
    pub join_type: Option<String>,
    pub filter: Option<String>,
    pub index_cond: Option<String>,
    pub hash_cond: Option<String>,
    pub join_filter: Option<String>,
    pub sort_key: Vec<String>,
    pub parallel: bool,
    pub startup_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub plan_rows: Option<f64>,
    pub plan_width: Option<i64>,
    pub actual_startup: Option<f64>,
    pub actual_total: Option<f64>,
    pub actual_rows: Option<f64>,
    pub loops: Option<f64>,
    /// Actual Total Time × Actual Loops：节点对父节点贡献的总毫秒
    pub inclusive_ms: Option<f64>,
    /// 包含耗时减去子节点包含耗时之和
    pub exclusive_ms: Option<f64>,
    pub rows_removed_by_filter: Option<f64>,
    pub temp_written: Option<f64>,
    pub shared_hit: Option<f64>,
    pub shared_read: Option<f64>,
    pub heap_fetches: Option<f64>,
    pub notes: Vec<Diag>,
    pub children: Vec<Node>,
}

#[derive(Debug, Serialize)]
pub struct Statement {
    pub index: usize,
    pub planning_time: Option<f64>,
    pub execution_time: Option<f64>,
    /// EXPLAIN 里保存的原始 SQL（若有）
    pub statement: Option<String>,
    pub has_actual: bool,
    pub root: Option<Node>,
    pub node_count: usize,
}

#[derive(Debug, Serialize)]
pub struct Hotspot {
    pub statement: usize,
    pub node_id: u32,
    pub label: String,
    /// time | spill | deviation
    pub kind: &'static str,
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub handle: u64,
    pub statements: Vec<Statement>,
    pub hotspots: Vec<Hotspot>,
    pub diagnostics: Vec<Diag>,
}

fn label_for(node: &RawNode) -> String {
    let mut label = node.node_type.clone();
    if let Some(relation) = &node.relation_name {
        let qualified = match &node.schema {
            Some(schema) => format!("{schema}.{relation}"),
            None => relation.clone(),
        };
        label.push_str(" on ");
        label.push_str(&qualified);
        if let Some(alias) = &node.alias
            && alias != relation
        {
            label.push_str(&format!(" as {alias}"));
        }
    }
    if node.parallel_aware {
        label.push_str(" (parallel)");
    }
    label
}

fn build_node(raw: &RawNode, depth: u32, state: &mut (usize, Limits)) -> Result<Node, PluginError> {
    let (count, limits) = state;
    *count += 1;
    if *count > limits.max_nodes {
        return Err(PluginError::new("pg.too_many_nodes").with("max", limits.max_nodes));
    }
    if depth as usize > limits.max_depth {
        return Err(PluginError::new("pg.too_deep").with("max", limits.max_depth));
    }
    let loops = raw.actual_loops;
    let inclusive_ms = raw
        .actual_total
        .map(|time| time * loops.unwrap_or(1.0).max(1.0));
    let mut children = Vec::with_capacity(raw.children.len());
    let mut child_inclusive = 0.0;
    for child in &raw.children {
        let built = build_node(child, depth + 1, state)?;
        if let Some(value) = built.inclusive_ms {
            child_inclusive += value;
        }
        children.push(built);
    }
    let exclusive_ms = inclusive_ms.map(|value| (value - child_inclusive).max(0.0));

    let mut notes = Vec::new();
    let has_actual = raw.actual_total.is_some();
    if has_actual {
        let actual_total_rows = raw.actual_rows.map(|rows| rows * loops.unwrap_or(1.0));
        if let (Some(plan_rows), Some(actual_total_rows)) = (raw.plan_rows, actual_total_rows) {
            let deviates = (actual_total_rows > plan_rows * DEVIATION_FACTOR
                && actual_total_rows >= DEVIATION_MIN_ROWS)
                || (plan_rows > actual_total_rows * DEVIATION_FACTOR
                    && plan_rows >= DEVIATION_MIN_ROWS);
            if deviates {
                notes.push(
                    Diag::new("pg.row_deviation")
                        .param("plan", plan_rows)
                        .param("actual", actual_total_rows),
                );
            }
        }
        if raw.node_type.contains("Sort") && raw.temp_written.unwrap_or(0.0) > 0.0 {
            notes.push(
                Diag::new("pg.sort_spilled").param("blocks", raw.temp_written.unwrap_or(0.0)),
            );
        }
        let rows_for_scan = actual_total_rows.unwrap_or(raw.plan_rows.unwrap_or(0.0));
        if raw.node_type == "Seq Scan" && rows_for_scan > SEQ_SCAN_ROWS {
            notes.push(Diag::new("pg.seq_scan_many_rows").param("rows", rows_for_scan));
        }
    }

    Ok(Node {
        id: 0,
        depth,
        label: label_for(raw),
        node_type: raw.node_type.clone(),
        relation: raw.relation_name.clone(),
        index_name: raw.index_name.clone(),
        join_type: raw.join_type.clone(),
        filter: raw.filter.clone(),
        index_cond: raw.index_cond.clone(),
        hash_cond: raw.hash_cond.clone(),
        join_filter: raw.join_filter.clone(),
        sort_key: raw.sort_key.clone(),
        parallel: raw.parallel_aware,
        startup_cost: raw.startup_cost,
        total_cost: raw.total_cost,
        plan_rows: raw.plan_rows,
        plan_width: raw.plan_width,
        actual_startup: raw.actual_startup,
        actual_total: raw.actual_total,
        actual_rows: raw.actual_rows,
        loops,
        inclusive_ms,
        exclusive_ms,
        rows_removed_by_filter: raw.rows_removed_by_filter,
        temp_written: raw.temp_written,
        shared_hit: raw.shared_hit,
        shared_read: raw.shared_read,
        heap_fetches: raw.heap_fetches,
        notes,
        children,
    })
}

fn assign_ids(node: &mut Node, next: &mut u32) {
    node.id = *next;
    *next += 1;
    for child in &mut node.children {
        assign_ids(child, next);
    }
}

fn flatten<'a>(node: &'a Node, out: &mut Vec<(&'a Node, usize)>) {
    out.push((node, node.depth as usize));
    for child in &node.children {
        flatten(child, out);
    }
}

fn build_statement(
    index: usize,
    raw: RawStatement,
    limits: &Limits,
) -> Result<Statement, PluginError> {
    let mut state = (0usize, *limits);
    let root = match raw.plan {
        Some(plan) => Some(build_node(&plan, 0, &mut state)?),
        None => None,
    };
    let mut root = root;
    if let Some(node) = &mut root {
        let mut next = 0u32;
        assign_ids(node, &mut next);
    }
    let has_actual = root
        .as_ref()
        .is_some_and(|node| node.actual_total.is_some());
    Ok(Statement {
        index,
        planning_time: raw.planning_time,
        execution_time: raw.execution_time,
        statement: raw.statement,
        has_actual,
        node_count: state.0,
        root,
    })
}

pub fn collect_hotspots(report: &Report) -> Vec<Hotspot> {
    let mut hotspots = Vec::new();
    for statement in &report.statements {
        let Some(root) = &statement.root else {
            continue;
        };
        let mut nodes = Vec::new();
        flatten(root, &mut nodes);
        for (node, _) in &nodes {
            // 热点按自身耗时排序；偏差与落盘提示已挂在节点 notes 上
            if let Some(exclusive) = node.exclusive_ms {
                hotspots.push(Hotspot {
                    statement: statement.index,
                    node_id: node.id,
                    label: node.label.clone(),
                    kind: "time",
                    value: exclusive,
                });
            }
        }
    }
    hotspots.sort_by(|a, b| {
        a.statement
            .cmp(&b.statement)
            .then(b.value.total_cmp(&a.value))
    });
    hotspots.truncate(HOTSPOT_LIMIT);
    hotspots
}

pub fn analyze_text(text: &str, limits: &Limits) -> Result<Report, PluginError> {
    let statements: Vec<RawStatement> = serde_json::from_str(text)
        .map_err(|e| PluginError::new("pg.invalid_json").with("detail", e.to_string()))?;
    if statements.is_empty() {
        return Err(PluginError::new("pg.empty"));
    }
    let mut built = Vec::new();
    for (index, raw) in statements.into_iter().enumerate() {
        built.push(build_statement(index, raw, limits)?);
    }
    let mut report = Report {
        handle: 0,
        statements: built,
        hotspots: Vec::new(),
        diagnostics: Vec::new(),
    };
    report.hotspots = collect_hotspots(&report);
    Ok(report)
}

/// 解析文件字节（load 任务读入后调用）
pub fn analyze_slice(bytes: &[u8], limits: &Limits) -> Result<Report, PluginError> {
    let statements: Vec<RawStatement> = serde_json::from_slice(bytes)
        .map_err(|e| PluginError::new("pg.invalid_json").with("detail", e.to_string()))?;
    if statements.is_empty() {
        return Err(PluginError::new("pg.empty"));
    }
    let mut built = Vec::new();
    for (index, raw) in statements.into_iter().enumerate() {
        built.push(build_statement(index, raw, limits)?);
    }
    let mut report = Report {
        handle: 0,
        statements: built,
        hotspots: Vec::new(),
        diagnostics: Vec::new(),
    };
    report.hotspots = collect_hotspots(&report);
    Ok(report)
}

impl Report {
    /// 全部语句的节点总数
    pub fn node_count(&self) -> usize {
        self.statements
            .iter()
            .map(|statement| statement.node_count)
            .sum()
    }
}

/// Markdown 报告：语句概览、热点与树形明细
pub fn markdown(report: &Report) -> String {
    let mut out = String::new();
    out.push_str("# PostgreSQL Plan Analysis\n\n");
    for statement in &report.statements {
        out.push_str(&format!("## Statement {}\n\n", statement.index + 1));
        if let Some(sql) = &statement.statement {
            out.push_str(&format!("```sql\n{sql}\n```\n\n"));
        }
        if let Some(time) = statement.execution_time {
            out.push_str(&format!("- Execution time: {time:.3} ms\n"));
        }
        if let Some(time) = statement.planning_time {
            out.push_str(&format!("- Planning time: {time:.3} ms\n"));
        }
        out.push_str(&format!(
            "- Actual metrics: {}\n- Nodes: {}\n\n",
            if statement.has_actual {
                "yes"
            } else {
                "no (plan only)"
            },
            statement.node_count,
        ));
        if let Some(root) = &statement.root {
            let mut nodes = Vec::new();
            flatten(root, &mut nodes);
            out.push_str("```text\n");
            for (node, _) in &nodes {
                let indent = "  ".repeat(node.depth as usize);
                let mut line = format!("{indent}{}{}", node.id, node.label);
                if let (Some(total), Some(rows), Some(loops)) =
                    (node.actual_total, node.actual_rows, node.loops)
                {
                    line.push_str(&format!(
                        "  [rows {rows:.0} x{loops:.0}, {total:.3} ms/loop]"
                    ));
                } else if let Some(cost) = node.total_cost {
                    line.push_str(&format!("  [cost {cost:.2}]"));
                }
                line.push('\n');
                out.push_str(&line);
                for note in &node.notes {
                    out.push_str(&format!("      ! {}\n", note.code));
                }
            }
            out.push_str("```\n\n");
        }
    }
    if !report.hotspots.is_empty() {
        out.push_str("## Hotspots\n\n");
        for hotspot in &report.hotspots {
            out.push_str(&format!(
                "- Statement {} node {} `{}` — {} ({:.3})\n",
                hotspot.statement + 1,
                hotspot.node_id,
                hotspot.label,
                hotspot.kind,
                hotspot.value,
            ));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "analysis_test.rs"]
mod tests;
