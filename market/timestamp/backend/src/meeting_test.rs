use super::*;

fn slot(day: i64, start: &str, end: &str) -> Slot {
    Slot {
        day,
        start: start.to_owned(),
        end: end.to_owned(),
        choices: Vec::new(),
    }
}

fn participant(label: &str, timezone: &str, slots: Vec<Slot>) -> Participant {
    Participant {
        label: label.to_owned(),
        timezone: timezone.to_owned(),
        slots,
    }
}

fn args(participants: Vec<Participant>, start_date: &str, duration: i64, days: i64) -> PlanArgs {
    PlanArgs {
        participants,
        base_timezone: "Asia/Shanghai".to_owned(),
        start_date: start_date.to_owned(),
        duration_minutes: duration,
        search_days: days,
    }
}

/// 2026-10-19 是周一
const MONDAY: &str = "2026-10-19";

#[test]
fn shanghai_new_york_london_overlap() {
    // 2026-10-19：上海 +8、伦敦 BST +1、纽约 EDT -4
    // 上海 09:00-18:00 = UTC 01:00-10:00；伦敦 09:00-18:00 = UTC 08:00-17:00；纽约 05:00-07:00 = UTC 09:00-11:00
    let out = plan(args(
        vec![
            participant("上海", "Asia/Shanghai", vec![slot(0, "09:00", "18:00")]),
            participant("伦敦", "Europe/London", vec![slot(0, "09:00", "18:00")]),
            participant("纽约", "America/New_York", vec![slot(0, "05:00", "07:00")]),
        ],
        MONDAY,
        30,
        1,
    ))
    .unwrap();
    assert_eq!(out.days[0].date, MONDAY);
    let windows = &out.days[0].windows;
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].start_minute, 17 * 60);
    assert_eq!(windows[0].end_minute, 18 * 60);
    assert_eq!(windows[0].rows.len(), 3);
    // 纽约行：上海 17:00 = EDT 05:00
    assert_eq!(windows[0].rows[2].start_minute, 5 * 60);
    assert!(out.text.contains("UTC"));
}

#[test]
fn half_hour_timezone_offset() {
    // 上海 +8 与加尔各答 +5:30：上海 09:00-12:00 = UTC 01:00-04:00 = 加尔各答 06:30-09:30
    let out = plan(args(
        vec![
            participant("上海", "Asia/Shanghai", vec![slot(0, "09:00", "12:00")]),
            participant("加尔各答", "Asia/Kolkata", vec![slot(0, "06:30", "09:30")]),
        ],
        MONDAY,
        30,
        1,
    ))
    .unwrap();
    let windows = &out.days[0].windows;
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].start_minute, 9 * 60);
    assert_eq!(windows[0].end_minute, 12 * 60);
    // 加尔各答行的本地分钟带半小时偏移
    let kolkata = &windows[0].rows[1];
    assert_eq!(kolkata.start_minute, 6 * 60 + 30);
    assert_eq!(kolkata.end_minute, 9 * 60 + 30);
}

#[test]
fn overnight_slot_crosses_midnight() {
    // 夜班 22:00-06:00（跨午夜），上海 08:00-09:00 与之交集只有 08:00-09:00
    let out = plan(args(
        vec![
            participant("白班", "Asia/Shanghai", vec![slot(0, "08:00", "09:00")]),
            participant("夜班", "Asia/Shanghai", vec![slot(1, "22:00", "06:00")]),
        ],
        MONDAY,
        30,
        2,
    ))
    .unwrap();
    // 夜班的周二 22:00 → 周三 06:00 与周三 08:00-09:00 无交集；周二 08:00-09:00 与周一 22:00-02:00 无交集
    assert!(out.days.iter().all(|day| day.windows.is_empty()));
    // 改为夜班 00:00-10:00：与周二 08:00-09:00 相交
    let out = plan(args(
        vec![
            participant("白班", "Asia/Shanghai", vec![slot(1, "08:00", "09:00")]),
            participant("夜班", "Asia/Shanghai", vec![slot(1, "00:00", "10:00")]),
        ],
        MONDAY,
        30,
        2,
    ))
    .unwrap();
    let windows: Vec<_> = out.days.iter().flat_map(|day| &day.windows).collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].start_minute, 8 * 60);
}

#[test]
fn cross_dateline_date_differs() {
    // 上海周一 19:00-21:00 = UTC 11:00-13:00 = 奥克兰（10 月 NZDT +13）周二 00:00-02:00
    let out = plan(args(
        vec![
            participant("上海", "Asia/Shanghai", vec![slot(0, "19:00", "21:00")]),
            participant(
                "奥克兰",
                "Pacific/Auckland",
                vec![slot(1, "00:00", "02:00")],
            ),
        ],
        MONDAY,
        30,
        1,
    ))
    .unwrap();
    let windows = &out.days[0].windows;
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].start_minute, 19 * 60);
    let auckland = &windows[0].rows[1];
    // 上海周一 19:00 = 奥克兰周二 00:00：当地日期已是周二
    assert_eq!(auckland.date, "2026-10-20");
    assert_eq!(auckland.weekday, 1);
    assert!(!auckland.cross_day);
}

#[test]
fn both_non_workday_window_found() {
    // 双方都只在周六可用：上海 10:00-12:00 = UTC 02:00-04:00；东京 11:00-12:00 = UTC 02:00-03:00
    let out = plan(args(
        vec![
            participant("甲", "Asia/Shanghai", vec![slot(5, "10:00", "12:00")]),
            participant("乙", "Asia/Tokyo", vec![slot(5, "11:00", "12:00")]),
        ],
        MONDAY,
        60,
        7,
    ))
    .unwrap();
    // 2026-10-24 是周六：交集上海 10:00-11:00（东京 11:00-12:00）
    let saturday = out
        .days
        .iter()
        .find(|day| day.date == "2026-10-24")
        .unwrap();
    assert_eq!(saturday.windows.len(), 1);
    assert_eq!(saturday.windows[0].start_minute, 10 * 60);
    assert_eq!(saturday.windows[0].end_minute, 11 * 60);
    // 其他 6 天无窗口
    assert_eq!(out.days.len(), 7);
    assert_eq!(out.days.iter().map(|d| d.windows.len()).sum::<usize>(), 1);
}

#[test]
fn no_common_window_and_too_long_meeting() {
    // 无交集
    let out = plan(args(
        vec![
            participant("甲", "Asia/Shanghai", vec![slot(0, "08:00", "09:00")]),
            participant("乙", "Asia/Shanghai", vec![slot(0, "10:00", "11:00")]),
        ],
        MONDAY,
        30,
        1,
    ))
    .unwrap();
    assert!(out.days[0].windows.is_empty());
    // 会议时长超过共同窗口 → 无完整容纳区间
    let out = plan(args(
        vec![
            participant("甲", "Asia/Shanghai", vec![slot(0, "08:00", "09:00")]),
            participant("乙", "Asia/Shanghai", vec![slot(0, "08:30", "10:00")]),
        ],
        MONDAY,
        60,
        1,
    ))
    .unwrap();
    assert!(out.days[0].windows.is_empty());
}

#[test]
fn spring_gap_is_reported_and_skipped() {
    // 美国 2026-03-08（周日）02:00 春令时切换：02:00 不存在，03:00 存在
    let out = plan(args(
        vec![participant(
            "纽约",
            "America/New_York",
            vec![slot(6, "02:00", "03:00")],
        )],
        "2026-03-02",
        30,
        7,
    ))
    .unwrap();
    assert_eq!(out.gaps.len(), 1);
    assert_eq!(out.gaps[0].date, "2026-03-08");
    assert_eq!(out.gaps[0].time, "02:00:00");
    assert!(out.days.iter().all(|day| day.windows.is_empty()));
}

#[test]
fn fall_ambiguity_lists_both_candidates_and_choice_respected() {
    // 美国 2026-11-01 02:00 秋令时回拨：01:00-01:30 出现两次
    let out = plan(args(
        vec![participant(
            "纽约",
            "America/New_York",
            vec![slot(6, "01:00", "01:30")],
        )],
        "2026-10-26",
        15,
        7,
    ))
    .unwrap();
    assert!(!out.ambiguities.is_empty());
    let first = &out.ambiguities[0];
    assert_eq!(first.earlier_offset, "-04:00");
    assert_eq!(first.later_offset, "-05:00");
    // 默认按 earlier 计算：EDT 01:00 = UTC 05:00
    let day = out.days.iter().find(|d| d.date == "2026-11-01").unwrap();
    assert_eq!(day.windows[0].utc_start, "2026-11-01 05:00");

    // 指定 later 消歧：EDT/EST 重叠的后一个 EST 01:00 = UTC 06:00
    let mut slot = slot(6, "01:00", "01:30");
    slot.choices.push(Choice {
        date: "2026-11-01".to_owned(),
        offset_seconds: -5 * 3600,
    });
    let out = plan(args(
        vec![participant("纽约", "America/New_York", vec![slot])],
        "2026-10-26",
        15,
        7,
    ))
    .unwrap();
    let day = out.days.iter().find(|d| d.date == "2026-11-01").unwrap();
    assert_eq!(day.windows[0].utc_start, "2026-11-01 06:00");
}

#[test]
fn meeting_across_dst_transition_uses_real_elapsed_time() {
    // 纽约 2026-03-08 春令时 02:00 跳到 03:00。凌晨窗口 01:00-04:00（EST→EDT）真实只有 2 小时
    let out = plan(args(
        vec![participant(
            "纽约",
            "America/New_York",
            vec![slot(6, "01:00", "04:00")],
        )],
        "2026-03-01",
        30,
        8,
    ))
    .unwrap();
    let day = out.days.iter().find(|d| d.date == "2026-03-08").unwrap();
    assert_eq!(day.windows.len(), 1);
    let window = &day.windows[0];
    // 基准时区（上海）显示 14:00-16:00，真实经过 120 分钟（纽约本地 01:00-04:00）
    assert_eq!(window.start_minute, 14 * 60);
    assert_eq!(window.end_minute, 16 * 60);
    assert_eq!(window.duration_minutes, 120);
    // 上一个周日（03-01）没有切换，真实 180 分钟
    let normal = out.days.iter().find(|d| d.date == "2026-03-01").unwrap();
    assert_eq!(normal.windows[0].duration_minutes, 180);
}

#[test]
fn dst_day_window_start_stays_correct() {
    // 切换日窗口在切换后开始：03:00-04:00 正常，真实 60 分钟（EDT）
    let out = plan(args(
        vec![participant(
            "纽约",
            "America/New_York",
            vec![slot(6, "03:00", "04:00")],
        )],
        "2026-03-02",
        30,
        8,
    ))
    .unwrap();
    let day = out.days.iter().find(|d| d.date == "2026-03-08").unwrap();
    assert_eq!(day.windows[0].duration_minutes, 60);
    assert_eq!(day.windows[0].utc_start, "2026-03-08 07:00");
}

#[test]
fn invalid_inputs_rejected() {
    let err = plan(args(vec![], MONDAY, 30, 1)).unwrap_err();
    assert_eq!(err.code, "meet.participants_range");
    let err = plan(args(
        vec![participant(
            "甲",
            "Asia/Shanghai",
            vec![slot(7, "08:00", "09:00")],
        )],
        MONDAY,
        30,
        1,
    ))
    .unwrap_err();
    assert_eq!(err.code, "meet.bad_weekday");
    let err = plan(args(
        vec![participant(
            "甲",
            "Asia/Shanghai",
            vec![slot(0, "08:00", "08:00")],
        )],
        MONDAY,
        30,
        1,
    ))
    .unwrap_err();
    assert_eq!(err.code, "meet.zero_slot");
    let err = plan(args(
        vec![participant(
            "甲",
            "Asia/Shanghai",
            vec![slot(0, "8:00", "09:00")],
        )],
        MONDAY,
        30,
        1,
    ))
    .unwrap_err();
    assert_eq!(err.code, "meet.bad_time");
    let err = plan(args(
        vec![participant(
            "甲",
            "Nowhere/Fake",
            vec![slot(0, "08:00", "09:00")],
        )],
        MONDAY,
        30,
        1,
    ))
    .unwrap_err();
    assert_eq!(err.code, "time.invalid_timezone");
    let err = plan(args(
        vec![participant(
            "甲",
            "Asia/Shanghai",
            vec![slot(0, "08:00", "09:00")],
        )],
        MONDAY,
        10,
        1,
    ))
    .unwrap_err();
    assert_eq!(err.code, "meet.duration_range");
    let err = plan(args(
        vec![participant(
            "甲",
            "Asia/Shanghai",
            vec![slot(0, "08:00", "09:00")],
        )],
        MONDAY,
        30,
        100,
    ))
    .unwrap_err();
    assert_eq!(err.code, "meet.search_range");
    let err = plan(args(
        vec![participant(
            "甲",
            "Asia/Shanghai",
            vec![slot(0, "08:00", "09:00")],
        )],
        "2026-13-40",
        30,
        1,
    ))
    .unwrap_err();
    assert_eq!(err.code, "meet.bad_date");
}

#[test]
fn empty_slots_yield_no_windows() {
    let out = plan(args(
        vec![
            participant("甲", "Asia/Shanghai", vec![]),
            participant("乙", "Asia/Tokyo", vec![slot(0, "08:00", "09:00")]),
        ],
        MONDAY,
        30,
        1,
    ))
    .unwrap();
    assert!(out.days[0].windows.is_empty());
}

#[test]
fn search_days_are_each_processed_with_their_own_dst() {
    // 悉尼 2026-10-04 02:00 进入夏令时（+10 → +11）。悉尼 09:00-09:30 与上海 05:00-08:00：
    // 切换前（09-27）= 上海 06:00-06:30；切换后（10-04）= 上海 05:00-05:30
    let out = plan(args(
        vec![
            participant("上海", "Asia/Shanghai", vec![slot(6, "05:00", "08:00")]),
            participant("悉尼", "Australia/Sydney", vec![slot(6, "09:00", "09:30")]),
        ],
        "2026-09-27",
        30,
        21,
    ))
    .unwrap();
    let before = out.days.iter().find(|d| d.date == "2026-09-27").unwrap();
    let after = out.days.iter().find(|d| d.date == "2026-10-04").unwrap();
    // 切换前（AEST +10）：悉尼 09:00 = 上海 07:00
    assert_eq!(before.windows[0].start_minute, 7 * 60);
    // 切换后（AEDT +11）：悉尼 09:00 = 上海 06:00
    assert_eq!(after.windows[0].start_minute, 6 * 60);
}
