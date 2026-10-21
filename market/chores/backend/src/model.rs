//! 数据模型：成员、家务（重复规则 + 轮换纪元）与一次 occurrence 的记录。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: String,
    pub name: String,
}

/// 重复规则：每天 / 每周指定星期 / 每隔若干天
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Rule {
    Daily,
    Weekly { weekdays: Vec<u8> },
    Interval { days: u32 },
}

/// 负责人模式：固定一人或按名单轮换
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Mode {
    Fixed {
        #[serde(rename = "memberId")]
        member_id: String,
    },
    Rotation {
        #[serde(rename = "memberIds")]
        member_ids: Vec<String>,
    },
}

/// 纪元：一次规则 / 名单修改从某天起生效；首纪元的 from 即首次日期。
/// 历史纪元只读，保证旧日期的负责人不随新修改漂移。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Epoch {
    pub from: String,
    pub rule: Rule,
    pub mode: Mode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chore {
    pub id: String,
    pub name: String,
    pub epochs: Vec<Epoch>,
    /// 停用日期：该日（含）之后不再排期；历史保留
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_from: Option<String>,
}

/// 一次 occurrence 的覆盖记录，键为 `choreId|YYYY-MM-DD`（稳定任务编号）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OccurrenceRecord {
    /// "done" 或 "skipped"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// 实际执行者（可能与计划负责人不同）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor: Option<String>,
    /// 实际完成日期：逾期补做时晚于计划日期
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_date: Option<String>,
    /// 临时换人：只影响这一次
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

pub type RecordMap = HashMap<String, OccurrenceRecord>;

/// 成员人数 / 家务数量 / 纪元数量上限
pub const MAX_MEMBERS: usize = 20;
pub const MAX_CHORES: usize = 50;
pub const MAX_EPOCHS: usize = 20;
