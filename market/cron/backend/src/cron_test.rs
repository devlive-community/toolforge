use super::*;

fn args(expression: &str) -> Args {
    Args {
        expression: expression.into(),
        timezone: "Asia/Shanghai".into(),
        count: 5,
        dom_and_dow: false,
    }
}

fn parts(expression: &str) -> Vec<Vec<Part>> {
    explain(expression)
        .unwrap()
        .1
        .into_iter()
        .map(|f| f.parts)
        .collect()
}

#[test]
fn explains_fields() {
    let p = parts("*/15 9-17 * JAN,jul MON-FRI");
    assert_eq!(
        p[0],
        vec![Part::Step {
            from: None,
            to: None,
            step: 15
        }]
    );
    assert_eq!(p[1], vec![Part::Range { from: 9, to: 17 }]);
    assert_eq!(p[2], vec![Part::Any]);
    assert_eq!(
        p[3],
        vec![Part::Value { value: 1 }, Part::Value { value: 7 }]
    );
    assert_eq!(p[4], vec![Part::Range { from: 1, to: 5 }]);
}

#[test]
fn explains_special_characters() {
    let p = parts("0 0 12 L,15W ? 5L,1#2");
    assert_eq!(p[3], vec![Part::LastDay, Part::NearestWeekday { day: 15 }]);
    assert_eq!(p[4], vec![Part::Any]);
    assert_eq!(
        p[5],
        vec![
            Part::LastOfMonth { weekday: 5 },
            Part::Nth { weekday: 1, nth: 2 }
        ]
    );
    assert_eq!(parts("0 0 LW * *")[2], vec![Part::LastWeekday]);
    assert_eq!(parts("0 0 * * 7")[4], vec![Part::Value { value: 7 }]);
    assert_eq!(parts("0 10-50/5 * * *")[0][0], Part::Value { value: 0 });
    assert_eq!(
        parts("0 10-50/5 * * *")[1],
        vec![Part::Step {
            from: Some(10),
            to: Some(50),
            step: 5
        }]
    );
}

#[test]
fn field_layouts_and_aliases() {
    let (normalized, fields) = explain("@daily").unwrap();
    assert_eq!(normalized, "0 0 * * *");
    assert_eq!(fields.len(), 5);
    let (_, six) = explain("  30   * * * * * ").unwrap();
    assert_eq!(six[0].field, Field::Second);
    assert_eq!(explain("0 0 1 1 * * 2030").unwrap().1[6].field, Field::Year);
    assert_eq!(explain("* * *").unwrap_err().code, "cron.field_count");
    assert_eq!(explain("  ").unwrap_err().code, "cron.empty");
    assert_eq!(explain("x * * * *").unwrap_err().code, "cron.invalid");
}

#[test]
fn computes_next_runs_in_timezone() {
    let report = evaluate(args("30 9 * * MON-FRI")).unwrap();
    assert_eq!(report.timezone, "Asia/Shanghai");
    assert_eq!(report.next.len(), 5);
    for run in &report.next {
        assert!(run.local.ends_with("09:30:00"), "{}", run.local);
        assert!((1..=5).contains(&run.weekday), "{}", run.weekday);
        assert!(run.in_seconds > 0);
    }
    assert!(
        report
            .next
            .windows(2)
            .all(|w| w[0].in_seconds < w[1].in_seconds)
    );
    assert!(report.previous.unwrap().in_seconds <= 0);
    assert!(!report.has_seconds);
}

#[test]
fn seconds_and_last_day() {
    let report = evaluate(args("*/20 * * * * *")).unwrap();
    assert!(report.has_seconds);
    assert!(report.next[1].in_seconds - report.next[0].in_seconds == 20);

    let last = evaluate(args("0 0 L * *")).unwrap();
    for run in &last.next {
        // 月末的下一天是 1 号
        let date = jiff::civil::Date::strptime("%Y-%m-%d", &run.local[..10]).unwrap();
        assert_eq!(date.tomorrow().unwrap().day(), 1, "{}", run.local);
    }
}

#[test]
fn accepts_quartz_style_steps() {
    let report = evaluate(args("0 0/15 8-18 * * *")).unwrap();
    assert!(report.has_seconds);
    assert!(report.next.iter().all(|r| r.local.ends_with(":00:00")
        || r.local.ends_with(":15:00")
        || r.local.ends_with(":30:00")
        || r.local.ends_with(":45:00")));
    assert_eq!(
        report.fields[1].parts,
        vec![Part::Step {
            from: Some(0),
            to: None,
            step: 15
        }]
    );
}

#[test]
fn reports_errors() {
    assert_eq!(
        evaluate(args("61 * * * *")).unwrap_err().code,
        "cron.invalid"
    );
    let mut tz = args("* * * * *");
    tz.timezone = "Mars/Olympus".into();
    assert_eq!(evaluate(tz).unwrap_err().code, "cron.unknown_timezone");
    // 2 月 30 日永远不会到来
    assert_eq!(evaluate(args("0 0 30 2 *")).unwrap_err().code, "cron.never");
}

#[test]
fn lists_timezones() {
    let zones = timezones();
    assert!(zones.iter().any(|z| z == "Asia/Shanghai"));
    assert!(zones.iter().any(|z| z == "UTC"));
}
