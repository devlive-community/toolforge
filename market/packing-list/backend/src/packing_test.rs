use super::*;

fn item(name: &str, base: i64, per_person: bool, per_day: bool) -> TemplateItem {
    TemplateItem {
        name: name.to_owned(),
        category: "general".to_owned(),
        unit: String::new(),
        base,
        per_person,
        per_day,
        note: None,
        weight: None,
    }
}

fn trip(items: Vec<TripItem>) -> Trip {
    Trip {
        people: 1,
        days: 1,
        bags: Vec::new(),
        items,
    }
}

fn trip_item(id: &str, target: i64, packed: i64) -> TripItem {
    TripItem {
        id: id.to_owned(),
        name: id.to_owned(),
        category: "general".to_owned(),
        unit: String::new(),
        target,
        packed,
        owner: None,
        bag_id: None,
        excluded: false,
        weight: None,
        note: None,
    }
}

#[test]
fn generate_applies_people_and_days_rules() {
    // 共享物品不自动翻倍；perPerson × 3 人、perDay × 2 天
    let items = generate(GenerateArgs {
        template: vec![
            item("充电宝", 1, false, false),
            item("袜子", 2, true, false),
            item("药", 1, false, true),
            item("口罩", 1, true, true),
        ],
        people: 3,
        days: 2,
    })
    .unwrap();
    assert_eq!(items[0].target, 1);
    assert_eq!(items[1].target, 6);
    assert_eq!(items[2].target, 2);
    assert_eq!(items[3].target, 6);
    // 生成的新物品从未装状态开始
    assert_eq!(items[0].packed, 0);
}

#[test]
fn generate_rejects_invalid_input() {
    let err = generate(GenerateArgs {
        template: vec![item("x", -1, false, false)],
        people: 1,
        days: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "plc.quantity_range");
    let err = generate(GenerateArgs {
        template: vec![item(" ", 1, false, false)],
        people: 1,
        days: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "plc.empty_name");
    let err = generate(GenerateArgs {
        template: vec![],
        people: 0,
        days: 1,
    })
    .unwrap_err();
    assert_eq!(err.code, "plc.people_range");
    let err = generate(GenerateArgs {
        template: vec![],
        people: 1,
        days: 400,
    })
    .unwrap_err();
    assert_eq!(err.code, "plc.days_range");
}

#[test]
fn summarize_tracks_progress_and_excluded_items() {
    let summary = summarize(&trip(vec![
        TripItem {
            packed: 2,
            ..trip_item("a", 3, 2)
        },
        trip_item("b", 1, 0),
        TripItem {
            excluded: true,
            ..trip_item("c", 1, 0)
        },
    ]))
    .unwrap();
    // 标记「不带」的项目不计入打包进度
    assert_eq!(summary.target, 4);
    assert_eq!(summary.packed, 2);
    assert_eq!(summary.excluded, 1);
    // a 差 1 件、b 未装 → 两条待装
    assert_eq!(summary.unpacked.len(), 2);
    assert_eq!(summary.unpacked[0].remaining, 1);
}

#[test]
fn packed_cannot_exceed_target() {
    let err = summarize(&trip(vec![trip_item("a", 1, 2)])).unwrap_err();
    assert_eq!(err.code, "plc.packed_range");
}

#[test]
fn weights_per_bag_and_unknown_counts() {
    let mut trip = trip(vec![
        TripItem {
            weight: Some(500),
            bag_id: Some("b1".to_owned()),
            ..trip_item("a", 2, 1)
        },
        TripItem {
            bag_id: Some("b1".to_owned()),
            ..trip_item("b", 1, 1)
        },
        TripItem {
            weight: Some(300),
            bag_id: Some("b2".to_owned()),
            ..trip_item("c", 1, 1)
        },
    ]);
    trip.bags = vec![
        Bag {
            id: "b1".to_owned(),
            name: "大箱".to_owned(),
            self_weight: Some(1200),
        },
        Bag {
            id: "b2".to_owned(),
            name: "背包".to_owned(),
            self_weight: None,
        },
    ];
    let summary = summarize(&trip).unwrap();
    // b1：500g 已装 + 未知重量的 b → packed 500、unknown 1、total 1700
    assert_eq!(summary.bags[0].packed_weight, 500);
    assert_eq!(summary.bags[0].unknown_weight_items, 1);
    assert_eq!(summary.bags[0].total_weight, Some(1700));
    // b2：自重未知 → total 为 null，不把未知当零
    assert_eq!(summary.bags[1].packed_weight, 300);
    assert_eq!(summary.bags[1].total_weight, None);
    assert_eq!(summary.total_weight, 800);
    assert_eq!(summary.unknown_weight_items, 1);
}

#[test]
fn cross_bag_split_counts_separately() {
    // 同一物品拆两个包：两条 TripItem 各自计数量，不重复计入完整件数
    let mut trip = trip(vec![
        TripItem {
            bag_id: Some("b1".to_owned()),
            ..trip_item("a-1", 2, 2)
        },
        TripItem {
            bag_id: Some("b2".to_owned()),
            ..trip_item("a-2", 1, 0)
        },
    ]);
    trip.bags = vec![
        Bag {
            id: "b1".to_owned(),
            name: String::new(),
            self_weight: None,
        },
        Bag {
            id: "b2".to_owned(),
            name: String::new(),
            self_weight: None,
        },
    ];
    let summary = summarize(&trip).unwrap();
    assert_eq!(summary.target, 3);
    assert_eq!(summary.packed, 2);
}

#[test]
fn unknown_bag_and_weight_ranges_rejected() {
    let mut trip_doc = trip(vec![trip_item("a", 1, 0)]);
    trip_doc.bags = vec![Bag {
        id: "b1".to_owned(),
        name: String::new(),
        self_weight: None,
    }];
    trip_doc.items[0].bag_id = Some("nope".to_owned());
    assert_eq!(summarize(&trip_doc).unwrap_err().code, "plc.unknown_bag");
    let err = summarize(&trip(vec![TripItem {
        weight: Some(-5),
        ..trip_item("a", 1, 0)
    }]))
    .unwrap_err();
    assert_eq!(err.code, "plc.weight_range");
}

#[test]
fn format_helpers() {
    assert_eq!(grams(500), "500 g");
    assert_eq!(grams(1200), "1.200 kg");
}
