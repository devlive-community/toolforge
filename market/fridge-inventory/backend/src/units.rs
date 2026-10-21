//! 单位登记表：g/kg、ml/L 同维换算在 Rust 完成；计数单位（包/个等）互不换算。

use serde::{Deserialize, Serialize};

/// (单位 id, 维度, 换算到基础单位的因子)；count 维度只允许同单位
const REGISTRY: &[(&str, &str, f64)] = &[
    ("g", "weight", 1.0),
    ("kg", "weight", 1000.0),
    ("ml", "volume", 1.0),
    ("l", "volume", 1000.0),
    ("piece", "count", 1.0),
    ("pack", "count", 1.0),
    ("bag", "count", 1.0),
    ("box", "count", 1.0),
    ("bottle", "count", 1.0),
    ("can", "count", 1.0),
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UnitError {
    /// 单位不在登记表
    Unknown,
    /// 跨维度或不同计数单位，拒绝猜测换算
    Mismatch,
}

fn entry(unit: &str) -> Option<(&'static str, f64)> {
    REGISTRY
        .iter()
        .find(|(id, _, _)| *id == unit)
        .map(|(_, dim, factor)| (*dim, *factor))
}

pub fn known(unit: &str) -> bool {
    entry(unit).is_some()
}

/// amount（单位 unit）→ 基础单位（g / ml / count）
pub fn to_base(amount: f64, unit: &str) -> Result<f64, UnitError> {
    let (_, factor) = entry(unit).ok_or(UnitError::Unknown)?;
    Ok(amount * factor)
}

/// 基础单位 → 目标单位
pub fn from_base(base: f64, unit: &str) -> Result<f64, UnitError> {
    let (_, factor) = entry(unit).ok_or(UnitError::Unknown)?;
    Ok(base / factor)
}

/// amount（单位 from）→ 单位 to；跨维度或不同计数单位拒绝
pub fn convert(amount: f64, from: &str, to: &str) -> Result<f64, UnitError> {
    let (from_dim, from_factor) = entry(from).ok_or(UnitError::Unknown)?;
    let (to_dim, to_factor) = entry(to).ok_or(UnitError::Unknown)?;
    if from_dim != to_dim || (from_dim == "count" && from != to) {
        return Err(UnitError::Mismatch);
    }
    Ok(amount * from_factor / to_factor)
}

#[cfg(test)]
#[path = "units_test.rs"]
mod tests;
