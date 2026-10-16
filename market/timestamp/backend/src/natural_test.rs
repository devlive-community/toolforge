use super::*;

/// 2025-10-15 是周三
fn now() -> Zoned {
    "2025-10-15T10:20:30+08:00[Asia/Shanghai]".parse().unwrap()
}

/// 每个候选：本地时间与说明
fn read(input: &str) -> Vec<String> {
    parse(input, &now())
        .unwrap_or_else(|| panic!("{input} not parsed"))
        .into_iter()
        .map(|c| {
            let at = c.at.strftime("%Y-%m-%d %H:%M:%S").to_string();
            if c.notes.is_empty() {
                at
            } else {
                format!("{at} {}", c.notes.join(","))
            }
        })
        .collect()
}

#[test]
fn chinese_relative_days_keep_the_current_time() {
    assert_eq!(read("明天"), ["2025-10-16 10:20:30"]);
    assert_eq!(read("后天"), ["2025-10-17 10:20:30"]);
    assert_eq!(read("大前天"), ["2025-10-12 10:20:30"]);
    assert_eq!(read("3天后"), ["2025-10-18 10:20:30"]);
    assert_eq!(read("两周前"), ["2025-10-01 10:20:30"]);
    assert_eq!(read("一个月后"), ["2025-11-15 10:20:30"]);
    assert_eq!(read("下个月"), ["2025-11-15 10:20:30"]);
    assert_eq!(read("明年"), ["2026-10-15 10:20:30"]);
    assert_eq!(read("现在"), ["2025-10-15 10:20:30"]);
}

#[test]
fn chinese_exact_offsets() {
    assert_eq!(read("3 小时后"), ["2025-10-15 13:20:30"]);
    assert_eq!(read("半小时后"), ["2025-10-15 10:50:30"]);
    assert_eq!(read("十五分钟前"), ["2025-10-15 10:05:30"]);
    // 精确偏移后不能再跟时刻
    assert!(parse("3小时后 9点", &now()).is_none());
}

#[test]
fn chinese_times_with_periods() {
    assert_eq!(read("明天上午9点"), ["2025-10-16 09:00:00"]);
    assert_eq!(read("明天下午三点半"), ["2025-10-16 15:30:00"]);
    assert_eq!(read("今晚8点"), ["2025-10-15 20:00:00"]);
    assert_eq!(read("后天 14:05"), ["2025-10-17 14:05:00"]);
    assert_eq!(read("中午12点15分"), ["2025-10-15 12:15:00"]);
    assert_eq!(read("中午1点"), ["2025-10-15 13:00:00"]);
    assert_eq!(read("凌晨12点"), ["2025-10-15 00:00:00"]);
    assert_eq!(read("明天中午"), ["2025-10-16 12:00:00"]);
    assert_eq!(read("下午四点一刻"), ["2025-10-15 16:15:00"]);
}

#[test]
fn hours_without_a_period_are_ambiguous() {
    assert_eq!(
        read("明天3点"),
        ["2025-10-16 03:00:00 am", "2025-10-16 15:00:00 pm"]
    );
    assert_eq!(read("明天15点"), ["2025-10-16 15:00:00"]);
}

#[test]
fn weekdays_offer_calendar_and_upcoming_readings() {
    // 今天周三：下周三 = 下一个自然周的周三，也就是接下来最近的周三，只有一种解释
    assert_eq!(read("下周三"), ["2025-10-22 10:20:30 nextWeek"]);
    // 下周一：自然周 10-20；接下来最近的周一也是 10-20
    assert_eq!(read("下周一 9:00"), ["2025-10-20 09:00:00 nextWeek"]);
    // 下周五：自然周 10-24；最近的周五是本周 10-17
    assert_eq!(
        read("下周五"),
        [
            "2025-10-24 10:20:30 nextWeek",
            "2025-10-17 10:20:30 upcoming"
        ]
    );
    // 只说周一：本周一已过，候选还有下一个周一
    assert_eq!(
        read("周一"),
        [
            "2025-10-13 10:20:30 thisWeek",
            "2025-10-20 10:20:30 upcoming"
        ]
    );
    assert_eq!(read("星期日"), ["2025-10-19 10:20:30"]);
    assert_eq!(read("上周五"), ["2025-10-10 10:20:30 lastWeek"]);
    assert_eq!(read("上上周一"), ["2025-09-29 10:20:30 weekBeforeLast"]);
    // 下周 + 时刻：数字是时刻不是星期
    assert_eq!(read("下周 3点")[0], "2025-10-22 03:00:00 am");
}

#[test]
fn english_expressions() {
    assert_eq!(read("tomorrow at 9am"), ["2025-10-16 09:00:00"]);
    assert_eq!(read("9:30 pm tomorrow"), ["2025-10-16 21:30:00"]);
    assert_eq!(read("in 2 hours"), ["2025-10-15 12:20:30"]);
    assert_eq!(read("in half an hour"), ["2025-10-15 10:50:30"]);
    assert_eq!(read("3 days ago"), ["2025-10-12 10:20:30"]);
    assert_eq!(read("a week from now"), ["2025-10-22 10:20:30"]);
    assert_eq!(read("next month"), ["2025-11-15 10:20:30"]);
    assert_eq!(read("Yesterday noon"), ["2025-10-14 12:00:00"]);
    assert_eq!(read("today midnight"), ["2025-10-15 00:00:00"]);
    assert_eq!(read("12am"), ["2025-10-15 00:00:00"]);
    assert_eq!(
        read("next friday 9"),
        [
            "2025-10-24 09:00:00 nextWeek,am",
            "2025-10-24 21:00:00 nextWeek,pm",
            "2025-10-17 09:00:00 upcoming,am",
            "2025-10-17 21:00:00 upcoming,pm",
        ]
    );
    assert_eq!(
        read("last monday"),
        [
            "2025-10-06 10:20:30 lastWeek",
            "2025-10-13 10:20:30 previous"
        ]
    );
}

#[test]
fn rejects_non_dates() {
    for input in [
        "hello",
        "17",
        "明天吃饭",
        "next",
        "13pm",
        "25:00",
        "下周八",
        "tomorrow banana",
    ] {
        assert!(parse(input, &now()).is_none(), "{input}");
    }
}

#[test]
fn chinese_numbers() {
    assert_eq!(number("十"), Some(10));
    assert_eq!(number("十二"), Some(12));
    assert_eq!(number("二十三"), Some(23));
    assert_eq!(number("两"), Some(2));
    assert_eq!(number("一百零五"), Some(105));
    assert_eq!(number("42"), Some(42));
    assert_eq!(number("abc"), None);
}
