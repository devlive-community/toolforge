use super::*;

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

fn args(batches: Vec<Batch>, movements: Vec<Movement>, op: Op) -> ApplyArgs {
    ApplyArgs {
        batches,
        movements,
        op,
        today: "2026-10-10".to_owned(),
    }
}

#[test]
fn deduct_converts_units_and_rejects_over_amount() {
    // 批次以 kg 登记，以 g 报消耗 300g = 0.3kg
    let out = apply(&args(
        vec![batch("b1", "牛肉", "kg", 1.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 300.0,
            unit: "g".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap();
    let movement = out.movement.unwrap();
    // 变动落账为批次单位 0.3kg
    assert!((movement.amount - 0.3).abs() < 1e-12);
    assert!((out.remaining - 0.7).abs() < 1e-12);

    // 超量拒绝：再扣 0.8kg > 0.7kg
    let error = apply(&args(
        vec![batch("b1", "牛肉", "kg", 1.0)],
        vec![movement.clone()],
        Op::Deduct {
            movement_id: "m2".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 0.8,
            unit: "kg".to_owned(),
            date: "2026-10-11".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.over_amount");
}

#[test]
fn deduct_rejects_bad_input_and_units() {
    // 非正数量
    let error = apply(&args(
        vec![batch("b1", "酸奶", "pack", 3.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 0.0,
            unit: "pack".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_amount");
    // 负数
    let error = apply(&args(
        vec![batch("b1", "酸奶", "pack", 3.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Discard,
            amount: -1.0,
            unit: "pack".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_amount");
    // 跨维单位：pack 批次报 500g
    let error = apply(&args(
        vec![batch("b1", "酸奶", "pack", 3.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 500.0,
            unit: "g".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.unit_mismatch");
    // adjust 不允许走扣减入口
    let error = apply(&args(
        vec![batch("b1", "酸奶", "pack", 3.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Adjust,
            amount: 1.0,
            unit: "pack".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_op");
    // 批次不存在
    let error = apply(&args(
        vec![],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "nope".to_owned(),
            kind: MovementKind::Consume,
            amount: 1.0,
            unit: "pack".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.batch_not_found");
    // 变动日期无效
    let error = apply(&args(
        vec![batch("b1", "酸奶", "pack", 3.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 1.0,
            unit: "pack".to_owned(),
            date: "2026-02-30".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.bad_date");
}

#[test]
fn set_intake_records_adjust_only_with_history() {
    // 无历史：直接改入库量，不留变动
    let out = apply(&args(
        vec![batch("b1", "鸡蛋", "piece", 10.0)],
        vec![],
        Op::SetIntake {
            batch_id: "b1".to_owned(),
            intake: 12.0,
            unit: "piece".to_owned(),
            date: "2026-10-10".to_owned(),
            movement_id: "m1".to_owned(),
        },
    ))
    .unwrap();
    assert!(out.movement.is_none());
    assert_eq!(out.batch.as_ref().unwrap().intake, 12.0);
    assert!((out.remaining - 12.0).abs() < 1e-12);

    // 有历史：留 adjust 记录（带符号 delta，批次单位），入库量本身不变
    let movements = vec![movement("m0", "b1", MovementKind::Consume, 4.0)];
    let out = apply(&args(
        vec![batch("b1", "鸡蛋", "piece", 10.0)],
        movements.clone(),
        Op::SetIntake {
            batch_id: "b1".to_owned(),
            intake: 15.0,
            unit: "piece".to_owned(),
            date: "2026-10-10".to_owned(),
            movement_id: "m1".to_owned(),
        },
    ))
    .unwrap();
    let adjust = out.movement.unwrap();
    assert_eq!(adjust.kind, MovementKind::Adjust);
    assert!((adjust.amount - 5.0).abs() < 1e-12);
    // 守恒：入库 10 + 调整 5 − 消耗 4 = 11
    assert!((out.remaining - 11.0).abs() < 1e-12);
    assert_eq!(out.batch.as_ref().unwrap().intake, 10.0);

    // 调整后余量为负 → 拒绝
    let error = apply(&args(
        vec![batch("b1", "鸡蛋", "piece", 10.0)],
        movements,
        Op::SetIntake {
            batch_id: "b1".to_owned(),
            intake: 3.0,
            unit: "piece".to_owned(),
            date: "2026-10-10".to_owned(),
            movement_id: "m1".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.over_amount");
}

#[test]
fn depleted_batch_rejects_further_deduction() {
    let movements = vec![movement("m0", "b1", MovementKind::Consume, 1.0)];
    let error = apply(&args(
        vec![batch("b1", "牛奶", "bottle", 1.0)],
        movements,
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 1.0,
            unit: "bottle".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.over_amount");
}

#[test]
fn movement_caps_are_enforced() {
    let movements: Vec<Movement> = (0..MAX_MOVEMENTS)
        .map(|i| movement(&format!("m{i}"), "b1", MovementKind::Consume, 0.0))
        .collect();
    let error = apply(&args(
        vec![batch("b1", "牛奶", "bottle", 1.0)],
        movements,
        Op::Deduct {
            movement_id: "mx".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 0.1,
            unit: "bottle".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap_err();
    assert_eq!(error.code, "fridge.too_many_movements");
}

#[test]
fn movement_dates_are_normalized() {
    let out = apply(&args(
        vec![batch("b1", "牛肉", "kg", 1.0)],
        vec![],
        Op::Deduct {
            movement_id: "m1".to_owned(),
            batch_id: "b1".to_owned(),
            kind: MovementKind::Consume,
            amount: 0.1,
            unit: "kg".to_owned(),
            date: "2026-10-10".to_owned(),
        },
    ))
    .unwrap();
    assert_eq!(
        out.movement.unwrap().date,
        format_date(parse_date("2026-10-10").unwrap())
    );
    assert_eq!(format_date(parse_date("2026-10-10").unwrap()), "2026-10-10");
}
