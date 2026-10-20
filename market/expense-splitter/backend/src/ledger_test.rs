use super::*;

fn members() -> Vec<Member> {
    vec![
        Member {
            id: "m1".to_owned(),
            name: "甲".to_owned(),
        },
        Member {
            id: "m2".to_owned(),
            name: "乙".to_owned(),
        },
        Member {
            id: "m3".to_owned(),
            name: "丙".to_owned(),
        },
    ]
}

fn equal(participants: &[&str]) -> Split {
    Split::Equal {
        participants: participants.iter().map(|id| id.to_string()).collect(),
    }
}

fn ledger(expenses: Vec<Expense>) -> Ledger {
    Ledger {
        members: members(),
        expenses,
        settlements: Vec::new(),
    }
}

fn expense(id: &str, payer: &str, total: &str, split: Split) -> Expense {
    Expense {
        id: id.to_owned(),
        title: id.to_owned(),
        date: None,
        total: total.to_owned(),
        payer: payer.to_owned(),
        split,
    }
}

fn allocation_of(report: &Report, expense_index: usize, member: &str) -> i64 {
    report.expenses[expense_index]
        .allocations
        .iter()
        .find(|a| a.id == member)
        .map(|a| a.cents)
        .unwrap_or_default()
}

fn balance_of<'a>(report: &'a Report, member: &str) -> &'a Balance {
    report.balances.iter().find(|b| b.id == member).unwrap()
}

#[test]
fn parse_cents_accepts_two_decimals_only() {
    assert_eq!(parse_cents("100", "esp.bad_total").unwrap(), 10000);
    assert_eq!(parse_cents("12.34", "esp.bad_total").unwrap(), 1234);
    assert_eq!(parse_cents("12.3", "esp.bad_total").unwrap(), 1230);
    assert_eq!(parse_cents("0.01", "esp.bad_total").unwrap(), 1);
    for bad in ["-1", "12.345", "abc", "", "1.2.3", "+5", "1 2"] {
        assert!(parse_cents(bad, "esp.bad_total").is_err(), "{bad}");
    }
}

#[test]
fn hundred_yuan_split_three_ways_distributes_remainder_in_order() {
    // 100 元三人分 → 余 1 分按成员顺序：甲 3334、乙 3333、丙 3333
    let report = settle(&ledger(vec![expense(
        "e1",
        "m1",
        "100",
        equal(&["m1", "m2", "m3"]),
    )]))
    .unwrap();
    assert_eq!(allocation_of(&report, 0, "m1"), 3334);
    assert_eq!(allocation_of(&report, 0, "m2"), 3333);
    assert_eq!(allocation_of(&report, 0, "m3"), 3333);
    let total: i64 = report.expenses[0].allocations.iter().map(|a| a.cents).sum();
    assert_eq!(total, 10000, "每笔份额之和必须等于总额");
}

#[test]
fn payer_can_be_outside_participants() {
    // 甲垫付但不参与分摊：甲 paid 100、owed 0
    let report = settle(&ledger(vec![expense(
        "e1",
        "m1",
        "100",
        equal(&["m2", "m3"]),
    )]))
    .unwrap();
    let balance = balance_of(&report, "m1");
    assert_eq!(balance.paid, 10000);
    assert_eq!(balance.owed, 0);
    assert_eq!(balance.net, 10000);
    // 转账建议：乙、丙 各补 甲 50 元
    assert_eq!(report.transfers.len(), 2);
    assert!(report.transfers.iter().all(|t| t.to == "m1"));
}

#[test]
fn shares_mode_distributes_remainder_by_weight() {
    // 100 元按份数 [2, 1]：unit 3333 余 1 → 份数 2 的甲补 1 分
    let report = settle(&ledger(vec![Expense {
        id: "e1".to_owned(),
        title: "e1".to_owned(),
        date: None,
        total: "100".to_owned(),
        payer: "m1".to_owned(),
        split: Split::Shares {
            weights: vec![
                Weight {
                    id: "m1".to_owned(),
                    shares: 2,
                },
                Weight {
                    id: "m2".to_owned(),
                    shares: 1,
                },
            ],
        },
    }]))
    .unwrap();
    assert_eq!(allocation_of(&report, 0, "m1"), 6667);
    assert_eq!(allocation_of(&report, 0, "m2"), 3333);
}

#[test]
fn exact_mode_requires_exact_sum() {
    // 精确金额不完整 → 拒绝
    let err = settle(&ledger(vec![Expense {
        id: "e1".to_owned(),
        title: "e1".to_owned(),
        date: None,
        total: "100".to_owned(),
        payer: "m1".to_owned(),
        split: Split::Exact {
            amounts: vec![
                Amount {
                    id: "m1".to_owned(),
                    cents: "50".to_owned(),
                },
                Amount {
                    id: "m2".to_owned(),
                    cents: "40".to_owned(),
                },
            ],
        },
    }]))
    .unwrap_err();
    assert_eq!(err.code, "esp.exact_sum");
    // 明确的零元允许：丙 0 元参与
    let report = settle(&ledger(vec![Expense {
        id: "e1".to_owned(),
        title: "e1".to_owned(),
        date: None,
        total: "100".to_owned(),
        payer: "m1".to_owned(),
        split: Split::Exact {
            amounts: vec![
                Amount {
                    id: "m1".to_owned(),
                    cents: "60".to_owned(),
                },
                Amount {
                    id: "m2".to_owned(),
                    cents: "40".to_owned(),
                },
                Amount {
                    id: "m3".to_owned(),
                    cents: "0".to_owned(),
                },
            ],
        },
    }]))
    .unwrap();
    assert_eq!(allocation_of(&report, 0, "m3"), 0);
    let sum: i64 = report.balances.iter().map(|b| b.net).sum();
    assert_eq!(sum, 0);
}

#[test]
fn cross_paying_expenses_balance_to_zero() {
    // 多笔交叉垫付：甲付车费 200（三人），乙付餐费 90（甲乙），丙付门票 60（乙丙）
    let report = settle(&ledger(vec![
        expense("e1", "m1", "200", equal(&["m1", "m2", "m3"])),
        expense("e2", "m2", "90", equal(&["m1", "m2"])),
        expense("e3", "m3", "60", equal(&["m2", "m3"])),
    ]))
    .unwrap();
    // 甲：paid 20000，owed 6667+4500=11167 → net +8833
    assert_eq!(balance_of(&report, "m1").paid, 20000);
    assert_eq!(balance_of(&report, "m1").owed, 6667 + 4500);
    assert_eq!(balance_of(&report, "m1").net, 20000 - 6667 - 4500);
    // 全体净余额之和为 0
    let sum: i64 = report.balances.iter().map(|b| b.net).sum();
    assert_eq!(sum, 0);
    // 转账建议守恒：转出总额 = 应收总额
    let transferred: i64 = report.transfers.iter().map(|t| t.cents).sum();
    let receivable: i64 = report.balances.iter().map(|b| b.net.max(0)).sum();
    assert_eq!(transferred, receivable);
}

#[test]
fn settlements_update_net_and_are_not_re_suggested() {
    let mut ledger = ledger(vec![expense("e1", "m1", "100", equal(&["m1", "m2", "m3"]))]);
    // 部分结算：乙已补给甲 20 元
    ledger.settlements.push(Settlement {
        id: "s1".to_owned(),
        from: "m2".to_owned(),
        to: "m1".to_owned(),
        cents: "20".to_owned(),
        date: None,
    });
    let report = settle(&ledger).unwrap();
    // 甲垫付 100 元、承担 33.34，乙转给甲 20 元：
    // 甲 net = 10000 − 3334 + 2000 = 8666；乙 net = −3333 − 2000 = −5333
    assert_eq!(balance_of(&report, "m1").net, 8666);
    assert_eq!(balance_of(&report, "m2").net, -5333);
    // 已结算部分不再出现在建议里
    let to_m1: i64 = report
        .transfers
        .iter()
        .filter(|t| t.to == "m1")
        .map(|t| t.cents)
        .sum();
    assert_eq!(to_m1, 8666);
}

#[test]
fn settlement_report_preserves_metadata_and_parsed_cents() {
    let mut activity = ledger(vec![]);
    activity.settlements = vec![
        Settlement {
            id: "s1".to_owned(),
            from: "m2".to_owned(),
            to: "m1".to_owned(),
            cents: "12.34".to_owned(),
            date: Some("2026-10-20".to_owned()),
        },
        Settlement {
            id: "s2".to_owned(),
            from: "m3".to_owned(),
            to: "m1".to_owned(),
            cents: "0.01".to_owned(),
            date: None,
        },
    ];
    let report = settle(&activity).unwrap();
    assert_eq!(report.settlements.len(), 2);
    let first = &report.settlements[0];
    assert_eq!(first.id, "s1");
    assert_eq!(first.from, "m2");
    assert_eq!(first.to, "m1");
    assert_eq!(first.cents, 1234);
    assert_eq!(first.date.as_deref(), Some("2026-10-20"));
    assert_eq!(report.settlements[1].id, "s2");
    assert_eq!(report.settlements[1].cents, 1);
    assert_eq!(report.settlements[1].date, None);
    assert_eq!(balance_of(&report, "m2").settled_out, first.cents);
    assert_eq!(balance_of(&report, "m3").settled_out, 1);
    assert_eq!(balance_of(&report, "m1").settled_in, 1235);
}

#[test]
fn settlement_records_require_valid_members_and_positive_amounts() {
    for (from, to, cents, code) in [
        ("missing", "m1", "1", "esp.unknown_member"),
        ("m2", "missing", "1", "esp.unknown_member"),
        ("m1", "m1", "1", "esp.self_settlement"),
        ("m2", "m1", "0", "esp.settlement_positive"),
        ("m2", "m1", "-1", "esp.bad_total"),
        ("m2", "m1", "1.234", "esp.bad_total"),
    ] {
        let mut activity = ledger(vec![]);
        activity.settlements.push(Settlement {
            id: "s1".to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            cents: cents.to_owned(),
            date: Some("2026-10-20".to_owned()),
        });
        assert_eq!(settle(&activity).unwrap_err().code, code);
    }
}

#[test]
fn same_name_members_are_distinct_by_id() {
    let mut ledger = ledger(vec![expense(
        "e1",
        "m1",
        "100",
        Split::Shares {
            weights: vec![
                Weight {
                    id: "m1".to_owned(),
                    shares: 1,
                },
                Weight {
                    id: "m2".to_owned(),
                    shares: 1,
                },
            ],
        },
    )]);
    // 乙改名后与甲同名：id 不同互不影响
    ledger.members[1].name = "甲".to_owned();
    let report = settle(&ledger).unwrap();
    assert_eq!(report.balances[0].name, "甲");
    assert_eq!(report.balances[1].name, "甲");
    assert_eq!(allocation_of(&report, 0, "m1"), 5000);
    assert_eq!(allocation_of(&report, 0, "m2"), 5000);
}

#[test]
fn invalid_input_is_rejected() {
    // 自己转自己
    let mut activity = ledger(vec![expense("e1", "m1", "100", equal(&["m1", "m2", "m3"]))]);
    activity.settlements.push(Settlement {
        id: "s1".to_owned(),
        from: "m1".to_owned(),
        to: "m1".to_owned(),
        cents: "10".to_owned(),
        date: None,
    });
    assert_eq!(settle(&activity).unwrap_err().code, "esp.self_settlement");
    // 未知成员
    let err = settle(&ledger(vec![expense(
        "e1",
        "m9",
        "100",
        equal(&["m1", "m2"]),
    )]))
    .unwrap_err();
    assert_eq!(err.code, "esp.unknown_member");
    // 金额为负
    let err = settle(&ledger(vec![expense("e1", "m1", "-1", equal(&["m1"]))])).unwrap_err();
    assert_eq!(err.code, "esp.bad_total");
    // 空参与者 / 全零份数
    let err = settle(&ledger(vec![expense("e1", "m1", "100", equal(&[]))])).unwrap_err();
    assert_eq!(err.code, "esp.empty_participants");
    let err = settle(&ledger(vec![Expense {
        id: "e1".to_owned(),
        title: "e1".to_owned(),
        date: None,
        total: "100".to_owned(),
        payer: "m1".to_owned(),
        split: Split::Shares {
            weights: vec![Weight {
                id: "m1".to_owned(),
                shares: 0,
            }],
        },
    }]))
    .unwrap_err();
    assert_eq!(err.code, "esp.shares_empty");
    // 集合内重复 id
    let err = settle(&ledger(vec![expense(
        "e1",
        "m1",
        "100",
        equal(&["m1", "m1"]),
    )]))
    .unwrap_err();
    assert_eq!(err.code, "esp.duplicate_entry");
}

#[test]
fn greedy_suggestions_reduce_duplicate_transfers() {
    // 甲独自参与三笔各自己付的费用：甲欠 300，乙丙各垫 60/40 → 两笔转账清账
    let report = settle(&ledger(vec![
        expense("e1", "m1", "200", equal(&["m1"])),
        expense("e2", "m2", "60", equal(&["m1"])),
        expense("e3", "m3", "40", equal(&["m1"])),
    ]))
    .unwrap();
    // 甲：paid 20000，owed 20000+6000+4000=30000 → net −10000
    assert_eq!(balance_of(&report, "m1").net, -10000);
    assert_eq!(balance_of(&report, "m2").net, 6000);
    assert_eq!(balance_of(&report, "m3").net, 4000);
    // 转账建议守恒：应收总额 = 乙丙补甲
    let transferred: i64 = report.transfers.iter().map(|t| t.cents).sum();
    assert_eq!(transferred, 10000);
    assert!(report.transfers.iter().all(|t| t.from == "m1"));
}
