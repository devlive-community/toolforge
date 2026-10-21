use super::*;

use crate::model::MAX_BATCHES;

fn batch(id: &str, name: &str, unit: &str, intake: f64) -> Batch {
    Batch {
        id: id.to_owned(),
        name: name.to_owned(),
        location: "fridge".to_owned(),
        unit: unit.to_owned(),
        intake,
        purchased_date: "2026-10-01".to_owned(),
        label_date: Some("2026-10-20".to_owned()),
        opened_date: None,
        attention_date: None,
    }
}

fn movement(id: &str, batch_id: &str, kind: MovementKind, amount: f64) -> Movement {
    Movement {
        id: id.to_owned(),
        batch_id: batch_id.to_owned(),
        kind,
        amount,
        date: "2026-10-05".to_owned(),
    }
}

#[test]
fn review_conserves_and_buckets_dates() {
    // 0.5 + 调整(−0.1) − 消耗(0.2) − 丢弃(0.1) = 0.1
    let batches = vec![Batch {
        label_date: Some("2026-10-11".to_owned()), // 明天 → soon（soon_days=3）
        attention_date: Some("2026-10-09".to_owned()), // 已过 → overdue
        opened_date: Some("2026-10-05".to_owned()),
        ..batch("b1", "酸奶", "kg", 0.5)
    }];
    let movements = vec![
        movement("m1", "b1", MovementKind::Adjust, -0.1),
        movement("m2", "b1", MovementKind::Consume, 0.2),
        movement("m3", "b1", MovementKind::Discard, 0.1),
    ];
    let out = review(&ReviewArgs {
        batches,
        movements,
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap();
    let view = &out.batches[0];
    assert!((view.remaining - 0.1).abs() < 1e-12);
    assert!((view.consumed - 0.2).abs() < 1e-12);
    assert!((view.discarded - 0.1).abs() < 1e-12);
    assert!((view.adjusted + 0.1).abs() < 1e-12);
    assert_eq!(view.status, "active");
    assert!(view.opened);
    assert_eq!(view.label_state, "soon");
    assert_eq!(view.attention_state, "overdue");
    assert_eq!(view.last_movement_id.as_deref(), Some("m3"));
    assert_eq!(view.movement_count, 3);
}

#[test]
fn review_marks_unknown_and_depleted() {
    let mut no_date = batch("b1", "冻肉", "pack", 2.0);
    no_date.label_date = None;
    let empty = batch("b2", "果汁", "bottle", 1.0);
    let out = review(&ReviewArgs {
        batches: vec![no_date, empty],
        movements: vec![movement("m1", "b2", MovementKind::Consume, 1.0)],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap();
    assert_eq!(out.batches[0].label_state, "unknown");
    assert_eq!(out.batches[0].status, "active");
    assert_eq!(out.batches[1].status, "depleted");
    assert!(out.batches[1].remaining.abs() < 1e-12);
    // 用完批次保留消耗历史
    assert_eq!(out.batches[1].movement_count, 1);
}

#[test]
fn review_validates_units_dates_and_counts() {
    // 未知单位
    let mut bad_unit = batch("b1", "x", "g", 1.0);
    bad_unit.unit = "cup".to_owned();
    let error = review(&ReviewArgs {
        batches: vec![bad_unit],
        movements: vec![],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_unit");
    // 入库量非正
    let error = review(&ReviewArgs {
        batches: vec![batch("b1", "x", "g", 0.0)],
        movements: vec![],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_amount");
    // 坏日期
    let mut bad_date = batch("b1", "x", "g", 1.0);
    bad_date.label_date = Some("2026-13-01".to_owned());
    let error = review(&ReviewArgs {
        batches: vec![bad_date],
        movements: vec![],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_date");
    // 批次超限
    let many: Vec<Batch> = (0..MAX_BATCHES + 1)
        .map(|i| batch(&format!("b{i}"), "x", "g", 1.0))
        .collect();
    let error = review(&ReviewArgs {
        batches: many,
        movements: vec![],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.too_many_batches");
    // 变动指向不存在的批次
    let error = review(&ReviewArgs {
        batches: vec![batch("b1", "x", "g", 1.0)],
        movements: vec![movement("m1", "ghost", MovementKind::Consume, 1.0)],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_movement");
    // 变动日期无效
    let error = review(&ReviewArgs {
        batches: vec![batch("b1", "x", "g", 1.0)],
        movements: vec![Movement {
            date: "2026/10/10".to_owned(),
            ..movement("m1", "b1", MovementKind::Consume, 1.0)
        }],
        today: "2026-10-10".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_date");
    // today 无效
    let error = review(&ReviewArgs {
        batches: vec![batch("b1", "x", "g", 1.0)],
        movements: vec![],
        today: "2026-10-1x".to_owned(),
        soon_days: 3,
    })
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_date");
}
