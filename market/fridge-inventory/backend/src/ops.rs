//! 数量事务：消耗 / 丢弃 / 调整入库量。余量不得为负，换算与校验都在 Rust 完成。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::{format_date, parse_date};
use crate::model::{
    Batch, EPS, MAX_AMOUNT, MAX_MOVEMENTS, MAX_MOVEMENTS_PER_BATCH, Movement, MovementKind,
};
use crate::review::remaining_base;
use crate::units;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyArgs {
    pub batches: Vec<Batch>,
    pub movements: Vec<Movement>,
    pub op: Op,
    pub today: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Op {
    /// 从指定批次消耗或丢弃数量（amount 以 unit 计）
    Deduct {
        #[serde(rename = "movementId")]
        movement_id: String,
        #[serde(rename = "batchId")]
        batch_id: String,
        kind: MovementKind,
        amount: f64,
        unit: String,
        date: String,
    },
    /// 修改入库量：有变动历史时留 adjust 记录，无历史直接改
    SetIntake {
        #[serde(rename = "batchId")]
        batch_id: String,
        intake: f64,
        unit: String,
        date: String,
        #[serde(rename = "movementId")]
        movement_id: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOut {
    /// 追加的变动；无历史调整入库量时为 null
    pub movement: Option<Movement>,
    /// 更新后的批次（无历史调整时改 intake）
    pub batch: Option<Batch>,
    pub remaining_base: f64,
    /// 剩余（批次单位）
    pub remaining: f64,
}

fn valid_amount(amount: f64) -> bool {
    amount.is_finite() && amount > 0.0 && amount <= MAX_AMOUNT
}

pub fn apply(args: &ApplyArgs) -> PluginResult<ApplyOut> {
    parse_date(&args.today).ok_or_else(|| errp("fridge.bad_date", "value", args.today.as_str()))?;
    if args.movements.len() >= MAX_MOVEMENTS {
        return Err(errp("fridge.too_many_movements", "limit", MAX_MOVEMENTS));
    }
    let op_batch_id = match &args.op {
        Op::Deduct { batch_id, .. } | Op::SetIntake { batch_id, .. } => batch_id,
    };
    let batch = args
        .batches
        .iter()
        .find(|b| b.id == *op_batch_id)
        .cloned()
        .ok_or_else(|| errp("fridge.batch_not_found", "hint", ""))?;
    if !units::known(&batch.unit) {
        return Err(errp("fridge.bad_unit", "value", batch.unit.as_str()));
    }
    let per_batch = args
        .movements
        .iter()
        .filter(|m| m.batch_id == batch.id)
        .count();
    let before = remaining_base(&batch, &args.movements);

    match &args.op {
        Op::Deduct {
            movement_id,
            kind,
            amount,
            unit,
            date,
            ..
        } => {
            if *kind == MovementKind::Adjust {
                return Err(errp("fridge.bad_op", "hint", "adjust"));
            }
            if !valid_amount(*amount) {
                return Err(errp("fridge.bad_amount", "value", *amount));
            }
            if movement_id.trim().is_empty() {
                return Err(errp("fridge.bad_op", "hint", "id"));
            }
            let day =
                parse_date(date).ok_or_else(|| errp("fridge.bad_date", "value", date.as_str()))?;
            // 换算到批次单位；跨维 / 不同计数单位拒绝
            let in_batch_unit =
                units::convert(*amount, unit, &batch.unit).map_err(|e| match e {
                    units::UnitError::Unknown => errp("fridge.bad_unit", "value", unit.as_str()),
                    units::UnitError::Mismatch => {
                        errp("fridge.unit_mismatch", "value", unit.as_str())
                    }
                })?;
            let base = units::to_base(in_batch_unit, &batch.unit).unwrap_or(f64::INFINITY);
            if !valid_amount(base) {
                return Err(errp("fridge.bad_amount", "value", *amount));
            }
            let after = before - base;
            if after < -EPS {
                return Err(errp(
                    "fridge.over_amount",
                    "remaining",
                    units::from_base(before, &batch.unit).unwrap_or(0.0),
                ));
            }
            if per_batch >= MAX_MOVEMENTS_PER_BATCH {
                return Err(errp(
                    "fridge.too_many_movements",
                    "limit",
                    MAX_MOVEMENTS_PER_BATCH,
                ));
            }
            let movement = Movement {
                id: movement_id.clone(),
                batch_id: batch.id.clone(),
                kind: *kind,
                amount: in_batch_unit,
                date: format_date(day),
            };
            Ok(ApplyOut {
                movement: Some(movement),
                batch: None,
                remaining_base: after,
                remaining: units::from_base(after, &batch.unit).unwrap_or(0.0),
            })
        }
        Op::SetIntake {
            movement_id,
            intake,
            unit,
            date,
            ..
        } => {
            if !valid_amount(*intake) {
                return Err(errp("fridge.bad_amount", "value", *intake));
            }
            if movement_id.trim().is_empty() {
                return Err(errp("fridge.bad_op", "hint", "id"));
            }
            let day =
                parse_date(date).ok_or_else(|| errp("fridge.bad_date", "value", date.as_str()))?;
            let new_in_batch_unit =
                units::convert(*intake, unit, &batch.unit).map_err(|e| match e {
                    units::UnitError::Unknown => errp("fridge.bad_unit", "value", unit.as_str()),
                    units::UnitError::Mismatch => {
                        errp("fridge.unit_mismatch", "value", unit.as_str())
                    }
                })?;
            let new_base = units::to_base(new_in_batch_unit, &batch.unit).unwrap_or(f64::INFINITY);
            if !new_base.is_finite() || new_base > MAX_AMOUNT {
                return Err(errp("fridge.bad_amount", "value", *intake));
            }
            let old_base = units::to_base(batch.intake, &batch.unit).unwrap_or(0.0);
            let after = before + new_base - old_base;
            if after < -EPS {
                return Err(errp(
                    "fridge.over_amount",
                    "remaining",
                    units::from_base(before, &batch.unit).unwrap_or(0.0),
                ));
            }
            let mut updated = batch.clone();
            let mut movement = None;
            if per_batch > 0 {
                // 有历史：调整留痕（带符号 delta，批次单位），入库量本身不动
                if per_batch >= MAX_MOVEMENTS_PER_BATCH {
                    return Err(errp(
                        "fridge.too_many_movements",
                        "limit",
                        MAX_MOVEMENTS_PER_BATCH,
                    ));
                }
                movement = Some(Movement {
                    id: movement_id.clone(),
                    batch_id: batch.id.clone(),
                    kind: MovementKind::Adjust,
                    amount: units::from_base(new_base - old_base, &batch.unit).unwrap_or(0.0),
                    date: format_date(day),
                });
            } else {
                updated.intake = new_in_batch_unit;
            }
            Ok(ApplyOut {
                movement,
                batch: Some(updated),
                remaining_base: after,
                remaining: units::from_base(after, &batch.unit).unwrap_or(0.0),
            })
        }
    }
}

#[cfg(test)]
#[path = "ops_test.rs"]
mod tests;
