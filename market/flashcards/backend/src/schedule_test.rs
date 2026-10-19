use super::*;

#[test]
fn first_pass_schedules_one_day() {
    // 新卡首次通过：连续 1 → 1 天档
    let out = review(ReviewArgs {
        stage: 0,
        today: "2026-10-19".to_owned(),
        remembered: true,
    })
    .unwrap();
    assert_eq!(out.stage, 1);
    assert_eq!(out.due, "2026-10-20");
    assert_eq!(out.interval, 1);
}

#[test]
fn streak_ladder_and_cap() {
    // 连续记住依次 3、7、14、30 天，之后保持 30
    let cases = [
        (1, "2026-10-19", "2026-10-22", 3),
        (2, "2026-10-19", "2026-10-26", 7),
        (3, "2026-10-19", "2026-11-02", 14),
        (4, "2026-10-19", "2026-11-18", 30),
        (5, "2026-10-19", "2026-11-18", 30),
        (9, "2026-10-19", "2026-11-18", 30),
    ];
    for (stage, today, due, interval) in cases {
        let out = review(ReviewArgs {
            stage,
            today: today.to_owned(),
            remembered: true,
        })
        .unwrap();
        assert_eq!(out.due, due, "stage {stage}");
        assert_eq!(out.interval, interval, "stage {stage}");
    }
}

#[test]
fn forget_resets_and_schedules_tomorrow() {
    // 没记住：回到 1 天档（明天），连续计数清零
    let out = review(ReviewArgs {
        stage: 4,
        today: "2026-10-19".to_owned(),
        remembered: false,
    })
    .unwrap();
    assert_eq!(out.stage, 0);
    assert_eq!(out.due, "2026-10-20");
    assert_eq!(out.interval, 1);
    // 清零后再记住又是首次 1 天
    let out = review(ReviewArgs {
        stage: 0,
        today: "2026-10-20".to_owned(),
        remembered: true,
    })
    .unwrap();
    assert_eq!(out.due, "2026-10-21");
    assert_eq!(out.interval, 1);
}

#[test]
fn cross_month_and_leap_day() {
    // 跨月：01-19 + 14 天 = 02-02
    let out = review(ReviewArgs {
        stage: 3,
        today: "2026-01-19".to_owned(),
        remembered: true,
    })
    .unwrap();
    assert_eq!(out.due, "2026-02-02");
    // 闰日：2024-02-28 + 1 天 = 2024-02-29；再 +1 天 = 03-01
    let out = review(ReviewArgs {
        stage: 0,
        today: "2024-02-28".to_owned(),
        remembered: true,
    })
    .unwrap();
    assert_eq!(out.due, "2024-02-29");
    let out = review(ReviewArgs {
        stage: 1,
        today: "2024-02-29".to_owned(),
        remembered: true,
    })
    .unwrap();
    assert_eq!(out.due, "2024-03-03"); // 连续第 2 次 → 3 天
    // 平年 2 月无 29 日
    let err = review(ReviewArgs {
        stage: 0,
        today: "2026-02-29".to_owned(),
        remembered: true,
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.bad_date");
}

#[test]
fn queue_selection_and_limits() {
    let card = |id: &str, stage: i64, due: &str, paused: bool| QueueCard {
        id: id.to_owned(),
        deck: "d".to_owned(),
        stage,
        due: due.to_owned(),
        paused,
    };
    let args = QueueArgs {
        cards: vec![
            card("new1", 0, "", false),
            card("new2", 0, "", false),
            card("due2", 2, "2026-10-18", false),
            card("due1", 1, "2026-10-19", false),
            card("future", 3, "2026-10-25", false),
            card("rest", 1, "2026-10-19", true),
        ],
        today: "2026-10-19".to_owned(),
        limit: 3,
        new_limit: 1,
    };
    let out = queue(args).unwrap();
    // 到期卡按到期日升序：due2（10-18）先于 due1（10-19）；future 不入选
    assert_eq!(out.due_ids, vec!["due2", "due1"]);
    // 总上限 3：到期 2 张占满后，新卡名额 = 3-2 = 1，但 new_limit=1 → new1
    assert_eq!(out.new_ids, vec!["new1"]);
    assert_eq!(out.counts.due, 2);
    assert_eq!(out.counts.new, 2);
    assert_eq!(out.counts.paused, 1);
    assert_eq!(out.counts.selected, 3);
    assert_eq!(out.next_due, "2026-10-25");
}

#[test]
fn queue_limit_goes_to_due_cards_first() {
    let card = |id: &str, due: &str| QueueCard {
        id: id.to_owned(),
        deck: "d".to_owned(),
        stage: 1,
        due: due.to_owned(),
        paused: false,
    };
    let args = QueueArgs {
        cards: vec![card("a", "2026-10-19"), card("b", "2026-10-19")],
        today: "2026-10-19".to_owned(),
        limit: 1,
        new_limit: 0,
    };
    let out = queue(args).unwrap();
    assert_eq!(out.due_ids, vec!["a"]);
    assert_eq!(out.counts.selected, 1);
    // 同日按 id 稳定排序
    let args = QueueArgs {
        cards: vec![card("b", "2026-10-19"), card("a", "2026-10-19")],
        today: "2026-10-19".to_owned(),
        limit: 2,
        new_limit: 0,
    };
    let out = queue(args).unwrap();
    assert_eq!(out.due_ids, vec!["a", "b"]);
}

#[test]
fn queue_rejects_invalid_limits_and_dates() {
    let card = QueueCard {
        id: "x".to_owned(),
        deck: String::new(),
        stage: 0,
        due: String::new(),
        paused: false,
    };
    let err = queue(QueueArgs {
        cards: vec![card.clone()],
        today: "2026-10-19".to_owned(),
        limit: 0,
        new_limit: 0,
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.limit_range");
    let err = queue(QueueArgs {
        cards: vec![card.clone()],
        today: "2026-10-19".to_owned(),
        limit: 10,
        new_limit: 11,
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.new_limit_range");
    let err = queue(QueueArgs {
        cards: vec![QueueCard {
            due: "2026-13-99".to_owned(),
            ..card.clone()
        }],
        today: "2026-10-19".to_owned(),
        limit: 10,
        new_limit: 0,
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.bad_date");
    let err = review(ReviewArgs {
        stage: -1,
        today: "2026-10-19".to_owned(),
        remembered: true,
    })
    .unwrap_err();
    assert_eq!(err.code, "fcs.bad_stage");
}
