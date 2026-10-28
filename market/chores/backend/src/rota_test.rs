use super::*;

fn args(rule: &str) -> RotaArgs {
    RotaArgs {
        members: vec!["Ann".into(), "Bo".into(), "Cy".into()],
        start: "2026-10-01".into(),
        rule: rule.into(),
        weekdays: vec![],
        every: 1,
        from: None,
        days: 5,
        name: "Dishes".into(),
    }
}

#[test]
fn daily_rotation_cycles_through_members() {
    let rows = rota(args("daily")).unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.member.as_str()).collect();
    assert_eq!(names, vec!["Ann", "Bo", "Cy", "Ann", "Bo"]);
    assert_eq!(rows[0].date, "2026-10-01");
}

#[test]
fn window_later_than_start_continues_the_rotation() {
    let mut a = args("daily");
    a.from = Some("2026-10-04".into());
    a.days = 2;
    let names: Vec<String> = rota(a).unwrap().into_iter().map(|r| r.member).collect();
    // 10-01 Ann, 10-02 Bo, 10-03 Cy, 10-04 Ann
    assert_eq!(names, vec!["Ann", "Bo"]);
}

#[test]
fn weekly_and_interval_rules() {
    let mut weekly = args("weekly");
    // 2026-10-01 是周四；只排周六（6）
    weekly.weekdays = vec![6];
    weekly.days = 14;
    let rows = rota(weekly).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.date.as_str()).collect::<Vec<_>>(),
        vec!["2026-10-03", "2026-10-10"]
    );
    let mut every = args("interval");
    every.every = 2;
    let rows = rota(every).unwrap();
    assert_eq!(rows.len(), 3);
}

#[test]
fn bad_input_is_rejected() {
    let mut a = args("daily");
    a.members = vec!["  ".into()];
    assert_eq!(rota(a).unwrap_err().code, "chore.rotation_required");
    assert_eq!(
        rota(args("weekly")).unwrap_err().code,
        "chore.weekday_required"
    );
    assert_eq!(rota(args("monthly")).unwrap_err().code, "chore.bad_rule");
    let mut long = args("daily");
    long.days = 1000;
    assert_eq!(rota(long).unwrap_err().code, "chore.bad_window");
}
