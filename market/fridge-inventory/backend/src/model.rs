//! 数据模型：批次与变动。批次按入库独立记录，不同日期不合并。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: String,
    pub name: String,
    /// 位置 id（来自用户的位置列表）
    pub location: String,
    /// 展示单位（登记表内）
    pub unit: String,
    /// 入库量（原始单位）；有历史后修改会以 adjust 变动留痕
    pub intake: f64,
    /// 入库日期
    pub purchased_date: String,
    /// 用户抄录的标签日期；未填 = 未知，不自动补保质期
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_date: Option<String>,
    /// 开封日期：只记录日期，不扣数量
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_date: Option<String>,
    /// 用户自设的开封后关注日，仅展示不结论
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention_date: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MovementKind {
    /// 入库量调整（带符号，批次单位）
    Adjust,
    Consume,
    Discard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Movement {
    pub id: String,
    pub batch_id: String,
    pub kind: MovementKind,
    /// 批次单位数量：consume/discard 为正数扣减，adjust 为带符号调整
    pub amount: f64,
    /// 变动发生日期
    pub date: String,
}

/// 守恒式：入库 + 调整 − 消耗 − 丢弃 = 剩余
pub const MAX_BATCHES: usize = 200;
pub const MAX_MOVEMENTS: usize = 3000;
pub const MAX_MOVEMENTS_PER_BATCH: usize = 300;
/// 数量上限（换算到基础单位后校验）
pub const MAX_AMOUNT: f64 = 1_000_000.0;
/// 剩余量判定为用完的容差
pub const EPS: f64 = 1e-9;
