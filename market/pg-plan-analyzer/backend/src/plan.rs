//! PostgreSQL EXPLAIN (FORMAT JSON) 的反序列化视图。
//!
//! 字段名带空格（如 "Node Type"），全部显式重命名并容缺：只有估计计划时
//! Actual*/Buffers 一类字段整体缺失；解析本身不连接数据库、不执行查询。

use serde::Deserialize;

#[derive(Deserialize)]
pub struct RawStatement {
    #[serde(default, rename = "Plan")]
    pub plan: Option<RawNode>,
    #[serde(default, rename = "Planning Time")]
    pub planning_time: Option<f64>,
    #[serde(default, rename = "Execution Time")]
    pub execution_time: Option<f64>,
    #[serde(default, rename = "Statement")]
    pub statement: Option<String>,
    // 其余字段（Triggers、Settings、JIT…）忽略
}

#[derive(Deserialize)]
pub struct RawNode {
    #[serde(rename = "Node Type")]
    pub node_type: String,
    #[serde(default, rename = "Parallel Aware")]
    pub parallel_aware: bool,
    #[serde(default, rename = "Relation Name")]
    pub relation_name: Option<String>,
    #[serde(default, rename = "Alias")]
    pub alias: Option<String>,
    #[serde(default, rename = "Schema")]
    pub schema: Option<String>,
    #[serde(default, rename = "Index Name")]
    pub index_name: Option<String>,
    #[serde(default, rename = "Join Type")]
    pub join_type: Option<String>,
    #[serde(default, rename = "Filter")]
    pub filter: Option<String>,
    #[serde(default, rename = "Index Cond")]
    pub index_cond: Option<String>,
    #[serde(default, rename = "Hash Cond")]
    pub hash_cond: Option<String>,
    #[serde(default, rename = "Join Filter")]
    pub join_filter: Option<String>,
    #[serde(default, rename = "Sort Key")]
    pub sort_key: Vec<String>,
    #[serde(default, rename = "Startup Cost")]
    pub startup_cost: Option<f64>,
    #[serde(rename = "Total Cost")]
    pub total_cost: Option<f64>,
    #[serde(default, rename = "Plan Rows")]
    pub plan_rows: Option<f64>,
    #[serde(default, rename = "Plan Width")]
    pub plan_width: Option<i64>,
    #[serde(default, rename = "Actual Startup Time")]
    pub actual_startup: Option<f64>,
    #[serde(default, rename = "Actual Total Time")]
    pub actual_total: Option<f64>,
    #[serde(default, rename = "Actual Rows")]
    pub actual_rows: Option<f64>,
    #[serde(default, rename = "Actual Loops")]
    pub actual_loops: Option<f64>,
    #[serde(default, rename = "Rows Removed by Filter")]
    pub rows_removed_by_filter: Option<f64>,
    #[serde(default, rename = "Shared Hit Blocks")]
    pub shared_hit: Option<f64>,
    #[serde(default, rename = "Shared Read Blocks")]
    pub shared_read: Option<f64>,
    #[serde(default, rename = "Temp Written Blocks")]
    pub temp_written: Option<f64>,
    #[serde(default, rename = "Heap Fetches")]
    pub heap_fetches: Option<f64>,
    #[serde(default, rename = "Plans")]
    pub children: Vec<RawNode>,
}

/// 读取文件大小的上限
pub const MAX_FILE: u64 = 64 * 1024 * 1024;
/// 粘贴文本的上限
pub const MAX_TEXT: usize = 32 * 1024 * 1024;
/// 节点数与树深上限
pub const MAX_NODES: usize = 50_000;
pub const MAX_DEPTH: usize = 200;

/// 数量限制；测试用更小的值
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_nodes: usize,
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: MAX_NODES,
            max_depth: MAX_DEPTH,
        }
    }
}
