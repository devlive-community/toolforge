use super::*;
use crate::model::{Epoch, Mode, OccurrenceRecord, Rule};

fn member(id: &str, name: &str) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}

fn epoch(from: &str, rule: Rule, mode: Mode) -> Epoch {
    Epoch {
        from: from.to_owned(),
        rule,
        mode,
    }
}

fn rotation(ids: &[&str]) -> Mode {
    Mode::Rotation {
        member_ids: ids.iter().map(|s| s.to_string()).collect(),
    }
}

fn chore(id: &str, name: &str, epochs: Vec<Epoch>) -> Chore {
    Chore {
        id: id.to_owned(),
        name: name.to_owned(),
        epochs,
        paused_from: None,
    }
}

fn args(
    members: Vec<Member>,
    chores: Vec<Chore>,
    records: RecordMap,
    from: &str,
    to: &str,
    today: &str,
) -> PlanArgs {
    PlanArgs {
        members,
        chores,
        records,
        today: today.to_owned(),
        from_date: from.to_owned(),
        to_date: to.to_owned(),
    }
}

#[test]
fn daily_rotation_advances_by_planned_occurrences() {
    let chores = vec![chore(
        "c1",
        "倒垃圾",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a", "b", "c"]))],
    )];
    let out = plan(&args(
        vec![
            member("a", "爸爸"),
            member("b", "妈妈"),
            member("c", "小明"),
        ],
        chores,
        RecordMap::new(),
        "2026-10-05",
        "2026-10-08",
        "2026-10-05",
    ))
    .unwrap();
    // 10-01 起每天一次：a b c a b c a b —— 10-05 是第 5 次（下标 4）→ b
    let plan_by_date: Vec<(&str, &str)> = out
        .occurrences
        .iter()
        .map(|o| (o.date.as_str(), o.assignee_name.as_deref().unwrap()))
        .collect();
    assert_eq!(
        plan_by_date,
        vec![
            ("2026-10-05", "妈妈"),
            ("2026-10-06", "小明"),
            ("2026-10-07", "爸爸"),
            ("2026-10-08", "妈妈")
        ]
    );
}

#[test]
fn late_checkoff_does_not_shift_rotation() {
    let chores = vec![chore(
        "c1",
        "洗碗",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a", "b"]))],
    )];
    let mut records = RecordMap::new();
    // 10-05（轮到 a）拖到 10-07 才补做
    records.insert(
        "c1|2026-10-05".to_owned(),
        OccurrenceRecord {
            status: Some("done".to_owned()),
            executor: Some("a".to_owned()),
            actual_date: Some("2026-10-07".to_owned()),
            assignee: None,
        },
    );
    let out = plan(&args(
        vec![member("a", "爸爸"), member("b", "妈妈")],
        chores,
        records,
        "2026-10-05",
        "2026-10-08",
        "2026-10-07",
    ))
    .unwrap();
    let by_date: HashMap<&str, &Occurrence> = out
        .occurrences
        .iter()
        .map(|o| (o.date.as_str(), o))
        .collect();
    let late = by_date["2026-10-05"];
    assert!(late.late, "补做要标 late");
    assert_eq!(late.status, "done");
    // 10-06 仍轮到 b、10-07 轮到 a：晚点打勾不改变其他日期的负责人
    assert_eq!(by_date["2026-10-06"].assignee_name.as_deref(), Some("妈妈"));
    assert_eq!(by_date["2026-10-07"].assignee_name.as_deref(), Some("爸爸"));
    // 10-05 已处理，不标 missed；10-06 未记录且在今天之前 → missed
    assert!(!late.missed);
    assert!(by_date["2026-10-06"].missed);
}

#[test]
fn weekly_interval_and_fixed_modes() {
    let chores = vec![
        chore(
            "w1",
            "周日大扫除",
            vec![epoch(
                "2026-10-01",
                Rule::Weekly { weekdays: vec![0] },
                rotation(&["a", "b"]),
            )],
        ),
        chore(
            "i1",
            "每隔三天浇花",
            vec![epoch(
                "2026-10-02",
                Rule::Interval { days: 3 },
                Mode::Fixed {
                    member_id: "b".to_owned(),
                },
            )],
        ),
    ];
    let out = plan(&args(
        vec![member("a", "爸爸"), member("b", "妈妈")],
        chores,
        RecordMap::new(),
        "2026-10-04",
        "2026-10-11",
        "2026-10-11",
    ))
    .unwrap();
    let sweep: Vec<&str> = out
        .occurrences
        .iter()
        .filter(|o| o.chore_id == "w1")
        .map(|o| o.date.as_str())
        .collect();
    // 窗口内只有 10-04 与 10-11 两个周日
    assert_eq!(sweep, vec!["2026-10-04", "2026-10-11"]);
    // 10-04 是 10-01 起第一个命中的周日 → a；10-11 → b
    let names: Vec<&str> = out
        .occurrences
        .iter()
        .filter(|o| o.chore_id == "w1")
        .map(|o| o.assignee_name.as_deref().unwrap())
        .collect();
    assert_eq!(names, vec!["爸爸", "妈妈"]);
    // 固定负责人：窗口内的 10-05、10-08、10-11 全是 b
    let flowers: Vec<&Occurrence> = out
        .occurrences
        .iter()
        .filter(|o| o.chore_id == "i1")
        .collect();
    assert_eq!(flowers.len(), 3);
    assert!(
        flowers
            .iter()
            .all(|o| o.assignee_name.as_deref() == Some("妈妈"))
    );
}

#[test]
fn rule_edit_only_affects_dates_from_selected_day() {
    // 前两周每天一次；从 10-15 起改为每周一三五，且换成两人名单
    let chores = vec![chore(
        "c1",
        "扫地",
        vec![
            epoch("2026-10-01", Rule::Daily, rotation(&["a", "b", "c"])),
            epoch(
                "2026-10-15",
                Rule::Weekly {
                    weekdays: vec![1, 3, 5],
                },
                rotation(&["b", "c"]),
            ),
        ],
    )];
    let members = vec![
        member("a", "爸爸"),
        member("b", "妈妈"),
        member("c", "小明"),
    ];
    // 旧区间：10-13（14 天窗口内第 13 次，下标 12 % 3 = 0 → a）
    let old = plan(&args(
        members.clone(),
        chores.clone(),
        RecordMap::new(),
        "2026-10-13",
        "2026-10-13",
        "2026-10-13",
    ))
    .unwrap();
    assert_eq!(old.occurrences[0].assignee_name.as_deref(), Some("爸爸"));
    // 新纪元首日 10-15（周四）不发生；10-16（周五）是新模式第 1 次 → b，10-19（周一）→ c
    let new = plan(&args(
        members.clone(),
        chores.clone(),
        RecordMap::new(),
        "2026-10-15",
        "2026-10-19",
        "2026-10-19",
    ))
    .unwrap();
    let dates: Vec<(&str, &str)> = new
        .occurrences
        .iter()
        .map(|o| (o.date.as_str(), o.assignee_name.as_deref().unwrap()))
        .collect();
    assert_eq!(dates, vec![("2026-10-16", "妈妈"), ("2026-10-19", "小明")]);
}

#[test]
fn pause_hides_from_paused_date_but_keeps_history() {
    let mut c = chore(
        "c1",
        "擦桌子",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a"]))],
    );
    c.paused_from = Some("2026-10-06".to_owned());
    let out = plan(&args(
        vec![member("a", "爸爸")],
        vec![c],
        RecordMap::new(),
        "2026-10-04",
        "2026-10-08",
        "2026-10-08",
    ))
    .unwrap();
    let dates: Vec<&str> = out.occurrences.iter().map(|o| o.date.as_str()).collect();
    assert_eq!(dates, vec!["2026-10-04", "2026-10-05"]);
}

#[test]
fn temp_reassignment_affects_only_one_occurrence() {
    let chores = vec![chore(
        "c1",
        "倒垃圾",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a", "b"]))],
    )];
    let mut records = RecordMap::new();
    records.insert(
        "c1|2026-10-05".to_owned(),
        OccurrenceRecord {
            assignee: Some("c".to_owned()),
            ..OccurrenceRecord::default()
        },
    );
    let out = plan(&args(
        vec![
            member("a", "爸爸"),
            member("b", "妈妈"),
            member("c", "小明"),
        ],
        chores,
        records,
        "2026-10-05",
        "2026-10-07",
        "2026-10-05",
    ))
    .unwrap();
    let by_date: HashMap<&str, &Occurrence> = out
        .occurrences
        .iter()
        .map(|o| (o.date.as_str(), o))
        .collect();
    assert_eq!(by_date["2026-10-05"].assignee_name.as_deref(), Some("小明"));
    assert!(by_date["2026-10-05"].overridden);
    assert_eq!(by_date["2026-10-06"].assignee_name.as_deref(), Some("妈妈"));
    assert!(!by_date["2026-10-06"].overridden);
}

#[test]
fn recheck_and_undo_are_idempotent() {
    let chores = vec![chore(
        "c1",
        "洗碗",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a", "b"]))],
    )];
    let mut records = RecordMap::new();
    // 重复点击完成：同一份记录覆盖写入，状态不变
    for _ in 0..3 {
        records.insert(
            "c1|2026-10-05".to_owned(),
            OccurrenceRecord {
                status: Some("done".to_owned()),
                executor: Some("a".to_owned()),
                actual_date: Some("2026-10-05".to_owned()),
                assignee: None,
            },
        );
    }
    let members = vec![member("a", "爸爸"), member("b", "妈妈")];
    let out = plan(&args(
        members.clone(),
        chores.clone(),
        records.clone(),
        "2026-10-05",
        "2026-10-06",
        "2026-10-05",
    ))
    .unwrap();
    assert_eq!(
        out.stats.iter().find(|s| s.member_id == "a").unwrap().done,
        1
    );
    // 撤销 = 删除记录键：pending 回来、done 归零
    records.remove("c1|2026-10-05");
    let out = plan(&args(
        members,
        chores,
        records,
        "2026-10-05",
        "2026-10-06",
        "2026-10-05",
    ))
    .unwrap();
    assert_eq!(out.occurrences[0].status, "pending");
    assert_eq!(
        out.stats.iter().find(|s| s.member_id == "a").unwrap().done,
        0
    );
}

#[test]
fn skip_counts_separately_from_done() {
    let chores = vec![chore(
        "c1",
        "倒垃圾",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a", "b"]))],
    )];
    let mut records = RecordMap::new();
    records.insert(
        "c1|2026-10-05".to_owned(),
        OccurrenceRecord {
            status: Some("done".to_owned()),
            executor: Some("b".to_owned()),
            actual_date: Some("2026-10-05".to_owned()),
            assignee: None,
        },
    );
    records.insert(
        "c1|2026-10-06".to_owned(),
        OccurrenceRecord {
            status: Some("skipped".to_owned()),
            assignee: None,
            ..OccurrenceRecord::default()
        },
    );
    let out = plan(&args(
        vec![member("a", "爸爸"), member("b", "妈妈")],
        chores,
        records,
        "2026-10-05",
        "2026-10-07",
        "2026-10-07",
    ))
    .unwrap();
    let a = out.stats.iter().find(|s| s.member_id == "a").unwrap();
    let b = out.stats.iter().find(|s| s.member_id == "b").unwrap();
    // 10-05 计划 a 但实际 b 做的：计入 b 的完成、a 的待做不变
    assert_eq!((a.done, a.skipped, a.pending), (0, 0, 1));
    assert_eq!((b.done, b.skipped), (1, 1));
}

#[test]
fn rejects_bad_windows_and_epoch_layout() {
    let members = vec![member("a", "爸爸")];
    let good = vec![chore(
        "c1",
        "x",
        vec![epoch("2026-10-01", Rule::Daily, rotation(&["a"]))],
    )];
    // 窗口倒挂
    let error = plan(&args(
        members.clone(),
        good.clone(),
        RecordMap::new(),
        "2026-10-10",
        "2026-10-01",
        "2026-10-10",
    ))
    .unwrap_err();
    assert_eq!(error.code, "chore.bad_window");
    // 窗口超长
    let error = plan(&args(
        members.clone(),
        good.clone(),
        RecordMap::new(),
        "2026-01-01",
        "2026-12-31",
        "2026-10-10",
    ))
    .unwrap_err();
    assert_eq!(error.code, "chore.bad_window");
    // 坏日期
    let error = plan(&args(
        members.clone(),
        good.clone(),
        RecordMap::new(),
        "2026-13-01",
        "2026-10-10",
        "2026-10-10",
    ))
    .unwrap_err();
    assert_eq!(error.code, "chore.bad_date");
    // 纪元乱序
    let bad = vec![chore(
        "c1",
        "x",
        vec![
            epoch("2026-10-15", Rule::Daily, rotation(&["a"])),
            epoch("2026-10-10", Rule::Daily, rotation(&["a"])),
        ],
    )];
    let error = plan(&args(
        members.clone(),
        bad,
        RecordMap::new(),
        "2026-10-10",
        "2026-10-16",
        "2026-10-16",
    ))
    .unwrap_err();
    assert_eq!(error.code, "chore.bad_epochs");
    // 空 epochs
    let error = plan(&args(
        members,
        vec![chore("c1", "x", vec![])],
        RecordMap::new(),
        "2026-10-10",
        "2026-10-11",
        "2026-10-11",
    ))
    .unwrap_err();
    assert_eq!(error.code, "chore.bad_epochs");
}

#[test]
fn window_is_bounded_for_backlog() {
    // 半年没打开：窗口只展开请求范围，长任务的量被上限截断
    let chores = vec![chore(
        "c1",
        "倒垃圾",
        vec![epoch("2026-01-01", Rule::Daily, rotation(&["a", "b"]))],
    )];
    let out = plan(&args(
        vec![member("a", "爸爸"), member("b", "妈妈")],
        chores,
        RecordMap::new(),
        "2026-10-01",
        "2026-10-20",
        "2026-10-20",
    ))
    .unwrap();
    assert_eq!(out.occurrences.len(), 20);
    assert!(out.occurrences.len() <= MAX_OCCURRENCES);
}
