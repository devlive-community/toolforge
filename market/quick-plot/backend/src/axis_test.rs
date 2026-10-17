use super::*;

fn labels(scale: &Scale) -> Vec<&str> {
    scale.ticks.iter().map(|t| t.label.as_str()).collect()
}

#[test]
fn picks_round_linear_ticks() {
    let scale = linear(3.2, 97.5, false, false);
    assert_eq!((scale.min, scale.max), (0.0, 100.0));
    assert_eq!(labels(&scale), ["0", "20", "40", "60", "80", "100"]);
    let small = linear(0.1, 0.35, false, false);
    assert_eq!(
        labels(&small),
        ["0.10", "0.15", "0.20", "0.25", "0.30", "0.35"]
    );
    // 柱状图包含 0；相同值也能给出范围
    assert_eq!(linear(50.0, 80.0, true, false).min, 0.0);
    let flat = linear(5.0, 5.0, false, false);
    assert!(flat.min < 5.0 && flat.max > 5.0);
    // 行号只用整数步长
    let index = linear(1.0, 3.0, false, true);
    assert!(index.ticks.iter().all(|t| t.value.fract() == 0.0));
    assert_eq!(format_number(-0.0001, 0.1), "0");
    assert_eq!(format_number(2e16, 1e15), "2e16");
}

#[test]
fn picks_decade_ticks_for_log_scales() {
    let wide = log(3.0, 25_000.0);
    assert_eq!((wide.min, wide.max), (1.0, 100_000.0));
    assert_eq!(labels(&wide), ["1", "10", "100", "1000", "10000", "100000"]);
    let narrow = log(0.02, 0.4);
    assert_eq!(
        labels(&narrow),
        ["0.01", "0.02", "0.05", "0.1", "0.2", "0.5", "1"]
    );
}

fn ms(text: &str) -> f64 {
    crate::infer::datetime(text).unwrap()
}

#[test]
fn aligns_time_ticks_in_local_time() {
    let scale = time(ms("2024-03-01 10:07:00"), ms("2024-03-01 11:52:00"));
    // 15 分钟一格，从整刻开始；第一个刻度带上日期
    assert_eq!(labels(&scale)[..3], ["03-01 10:15", "10:30", "10:45"]);
    let days = time(ms("2024-03-01 08:00:00"), ms("2024-03-09 20:00:00"));
    // 8.5 天按 2 天一格，从当天零点起算
    assert_eq!(labels(&days)[..2], ["03-03", "03-05"]);
    let months = time(ms("2023-11-15"), ms("2024-08-20"));
    assert!(labels(&months).contains(&"2024-01"));
    let years = time(ms("2001-06-01"), ms("2024-06-01"));
    assert!(labels(&years).iter().all(|l| l.len() == 4));
    let seconds = time(ms("2024-03-01 10:00:00"), ms("2024-03-01 10:00:30"));
    assert_eq!(labels(&seconds)[0], "03-01 10:00:00");
    let millis = time(ms("2024-03-01 10:00:00"), ms("2024-03-01 10:00:00") + 400.0);
    assert!(labels(&millis)[1].ends_with(".100") || labels(&millis)[1].ends_with(".050"));
    // 跨天的小时刻度在零点带日期
    let cross = time(ms("2024-03-01 20:00:00"), ms("2024-03-02 06:00:00"));
    assert!(labels(&cross).contains(&"03-02 00:00"));
}

#[test]
fn thins_category_labels() {
    let names: Vec<String> = (0..100).map(|i| format!("c{i}")).collect();
    let scale = categories(&names);
    assert!(scale.ticks.len() <= 24);
    assert_eq!(scale.ticks[0].label, "c0");
    assert_eq!((scale.min, scale.max), (-0.5, 99.5));
}

#[test]
fn shortens_time_category_labels() {
    let v = |texts: &[&str]| texts.iter().map(|t| Some(ms(t))).collect::<Vec<_>>();
    assert_eq!(
        time_labels(&v(&["2024-06-01 10:00:00", "2024-06-01 10:00:10"])),
        ["10:00:00", "10:00:10"]
    );
    assert_eq!(
        time_labels(&v(&["2024-06-01 10:00:00", "2024-06-01 11:30:00"])),
        ["10:00", "11:30"]
    );
    assert_eq!(
        time_labels(&v(&["2024-06-01", "2024-06-02"])),
        ["06-01", "06-02"]
    );
    assert_eq!(
        time_labels(&v(&["2023-12-31", "2024-01-01"])),
        ["2023-12-31", "2024-01-01"]
    );
    assert_eq!(
        time_labels(&v(&["2024-06-01 10:00:00", "2024-06-02 10:00:00"])),
        ["06-01 10:00:00", "06-02 10:00:00"].map(|l| l.replace(":00:00", ":00"))
    );
    assert_eq!(
        time_labels(&[None, Some(ms("2024-06-01 10:00:05"))]),
        ["", "10:00:05"]
    );
}
