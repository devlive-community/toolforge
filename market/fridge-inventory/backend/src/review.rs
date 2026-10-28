//! 汇总计算：余量守恒与日期分档。只描述状态，不给「还能吃」的结论。

use serde::{Deserialize, Serialize};
use tf_plugin_api::{PluginError, PluginResult};

use crate::date::parse_date;
use crate::model::{Batch, EPS, MAX_AMOUNT, MAX_BATCHES, MAX_MOVEMENTS, Movement, MovementKind};
use crate::units;

fn errp(code: &str, key: &str, value: impl Into<serde_json::Value>) -> PluginError {
    PluginError::new(code).with(key, value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewArgs {
    pub batches: Vec<Batch>,
    pub movements: Vec<Movement>,
    pub today: String,
    /// 用户设置的临近日期提前天数
    pub soon_days: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchView {
    #[serde(flatten)]
    pub batch: Batch,
    /// 剩余（批次单位）
    pub remaining: f64,
    pub remaining_base: f64,
    /// 累计消耗 / 丢弃（批次单位）
    pub consumed: f64,
    pub discarded: f64,
    /// 调整合计（批次单位，带符号）
    pub adjusted: f64,
    /// active | depleted
    pub status: String,
    pub opened: bool,
    /// 标签日期：overdue | soon | ok | unknown
    pub label_state: String,
    /// 自定关注日：none | overdue | soon | ok
    pub attention_state: String,
    /// 最近一笔变动（撤销入口）
    pub last_movement_id: Option<String>,
    pub movement_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOut {
    pub batches: Vec<BatchView>,
}

/// 守恒：入库 + 调整 − 消耗 − 丢弃 = 剩余（基础单位）
pub fn remaining_base(batch: &Batch, movements: &[Movement]) -> f64 {
    let intake = units::to_base(batch.intake, &batch.unit).unwrap_or(0.0);
    movements
        .iter()
        .filter(|m| m.batch_id == batch.id)
        .fold(intake, |acc, m| {
            let amount = units::to_base(m.amount, &batch.unit).unwrap_or(0.0);
            match m.kind {
                MovementKind::Adjust => acc + amount,
                MovementKind::Consume | MovementKind::Discard => acc - amount,
            }
        })
}

pub fn date_state(day: i64, today: i64, soon_days: u32) -> String {
    if day < today {
        "overdue".to_owned()
    } else if day <= today + i64::from(soon_days) {
        "soon".to_owned()
    } else {
        "ok".to_owned()
    }
}

pub fn review(args: &ReviewArgs) -> PluginResult<ReviewOut> {
    if args.batches.len() > MAX_BATCHES {
        return Err(errp("fridge.too_many_batches", "limit", MAX_BATCHES));
    }
    if args.movements.len() > MAX_MOVEMENTS {
        return Err(errp("fridge.too_many_movements", "limit", MAX_MOVEMENTS));
    }
    let today = parse_date(&args.today)
        .ok_or_else(|| errp("fridge.bad_date", "value", args.today.as_str()))?;
    for batch in &args.batches {
        if !units::known(&batch.unit) {
            return Err(errp("fridge.bad_unit", "value", batch.unit.as_str()));
        }
        if !batch.intake.is_finite() || batch.intake <= 0.0 || batch.intake > MAX_AMOUNT {
            return Err(errp("fridge.bad_amount", "batch", batch.name.as_str()));
        }
        parse_date(&batch.purchased_date)
            .ok_or_else(|| errp("fridge.bad_date", "value", batch.purchased_date.as_str()))?;
        for text in [&batch.label_date, &batch.attention_date, &batch.opened_date]
            .into_iter()
            .flatten()
        {
            parse_date(text).ok_or_else(|| errp("fridge.bad_date", "value", text.as_str()))?;
        }
    }
    for movement in &args.movements {
        parse_date(&movement.date)
            .ok_or_else(|| errp("fridge.bad_date", "value", movement.date.as_str()))?;
        if !args.batches.iter().any(|b| b.id == movement.batch_id) {
            return Err(errp("fridge.bad_movement", "id", movement.id.as_str()));
        }
    }

    let batches = args
        .batches
        .iter()
        .map(|batch| {
            let own: Vec<&Movement> = args
                .movements
                .iter()
                .filter(|m| m.batch_id == batch.id)
                .collect();
            let remaining_base = remaining_base(batch, &args.movements);
            let sum = |kind: MovementKind| -> f64 {
                let base: f64 = own
                    .iter()
                    .filter(|m| m.kind == kind)
                    .map(|m| units::to_base(m.amount, &batch.unit).unwrap_or(0.0))
                    .sum();
                units::from_base(base, &batch.unit).unwrap_or(0.0)
            };
            let status = if remaining_base <= EPS {
                "depleted"
            } else {
                "active"
            };
            let label_state = match &batch.label_date {
                None => "unknown".to_owned(),
                Some(text) => date_state(parse_date(text).unwrap_or(today), today, args.soon_days),
            };
            let attention_state = match &batch.attention_date {
                None => "none".to_owned(),
                Some(text) => date_state(parse_date(text).unwrap_or(today), today, args.soon_days),
            };
            let opened = batch.opened_date.is_some();
            BatchView {
                batch: batch.clone(),
                remaining: units::from_base(remaining_base, &batch.unit).unwrap_or(0.0),
                remaining_base,
                consumed: sum(MovementKind::Consume),
                discarded: sum(MovementKind::Discard),
                adjusted: sum(MovementKind::Adjust),
                status: status.to_owned(),
                opened,
                label_state,
                attention_state,
                last_movement_id: own.last().map(|m| m.id.clone()),
                movement_count: own.len(),
            }
        })
        .collect();

    Ok(ReviewOut { batches })
}

#[cfg(test)]
#[path = "review_test.rs"]
mod tests;
