use super::*;
use serde_json::json;

#[test]
fn schedules_beats_with_accents_and_subdivisions() {
    let out = schedule(ScheduleArgs {
        bpm: 120.0,
        signature: "4/4".into(),
        subdivision: Some(2),
        bars: Some(2),
        accents: vec![],
        count_in: None,
    })
    .unwrap();
    assert_eq!(out.beat_ms, 500);
    assert_eq!(out.beats.len(), 2 * 4 * 2);
    // 小节首拍重音
    assert_eq!(out.beats[0].level, 2);
    assert_eq!(out.beats[0].beat, 1);
    assert_eq!(out.beats[0].sub, 1);
    // 分拍时刻
    assert_eq!(out.beats[1].time_ms, 250);
    assert_eq!(out.beats[1].sub, 2);
    // 第二小节从 2000ms 开始；小节号从 1 起
    assert_eq!(out.beats[8].time_ms, 2000);
    assert_eq!(out.beats[8].bar, 2);
}

#[test]
fn six_eight_counts_six_beats() {
    let out = schedule(ScheduleArgs {
        bpm: 90.0,
        signature: "6/8".into(),
        subdivision: None,
        bars: Some(1),
        accents: vec![],
        count_in: None,
    })
    .unwrap();
    assert_eq!(out.beats_per_bar, 6);
    assert_eq!(out.beats.len(), 6);
}

#[test]
fn count_in_marks_prep_bar() {
    let out = schedule(ScheduleArgs {
        bpm: 60.0,
        signature: "3/4".into(),
        subdivision: None,
        bars: Some(1),
        accents: vec![],
        count_in: Some(1),
    })
    .unwrap();
    assert_eq!(out.beats.len(), 6);
    assert_eq!(out.beats[0].bar, 0, "count-in bar reads as 0");
    assert_eq!(out.beats[0].level, 2);
    assert_eq!(out.beats[3].bar, 1, "real music starts at bar 1");
    assert_eq!(out.beats[3].time_ms, 3000);
}

#[test]
fn custom_accent_levels_apply_per_beat() {
    let out = schedule(ScheduleArgs {
        bpm: 100.0,
        signature: "4/4".into(),
        subdivision: None,
        bars: Some(1),
        accents: vec![0, 1, 2, 1],
        count_in: None,
    })
    .unwrap();
    let levels: Vec<u8> = out.beats.iter().map(|b| b.level).collect();
    assert_eq!(levels, vec![0, 1, 2, 1]);
}

#[test]
fn rejects_bad_params() {
    let err = schedule(ScheduleArgs {
        bpm: 500.0,
        signature: "4/4".into(),
        subdivision: None,
        bars: None,
        accents: vec![],
        count_in: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "metronome.bad_bpm");
    let err = schedule(ScheduleArgs {
        bpm: 100.0,
        signature: "7/8".into(),
        subdivision: None,
        bars: None,
        accents: vec![],
        count_in: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "metronome.bad_signature");
    let err = schedule(ScheduleArgs {
        bpm: 100.0,
        signature: "4/4".into(),
        subdivision: Some(5),
        bars: None,
        accents: vec![],
        count_in: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "metronome.bad_subdivision");
    let err = schedule(ScheduleArgs {
        bpm: 100.0,
        signature: "4/4".into(),
        subdivision: None,
        bars: Some(100),
        accents: vec![],
        count_in: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "metronome.bad_bars");
}

#[test]
fn practice_plan_stages_to_target() {
    let out = practice_plan(PracticeArgs {
        start_bpm: 60.0,
        target_bpm: 72.0,
        every_bars: Some(4),
        step: Some(4.0),
    })
    .unwrap();
    let bpms: Vec<f64> = out.stages.iter().map(|s| s.bpm).collect();
    assert_eq!(bpms, vec![60.0, 64.0, 68.0, 72.0]);
    assert_eq!(
        out.stages.last().unwrap().bars,
        4,
        "target stage holds at least 4 bars"
    );
    let err = practice_plan(PracticeArgs {
        start_bpm: 100.0,
        target_bpm: 80.0,
        every_bars: None,
        step: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "metronome.start_above_target");
}

#[test]
fn dispatches_via_plugin() {
    let out = Metronome::default()
        .call(
            "schedule",
            json!({"bpm": 120, "signature": "4/4", "bars": 1}),
        )
        .unwrap();
    assert_eq!(out["beats"].as_array().unwrap().len(), 4);
    let out = Metronome::default()
        .call(
            "practice_plan",
            json!({"startBpm": 60, "targetBpm": 64, "step": 4}),
        )
        .unwrap();
    assert_eq!(out["stages"].as_array().unwrap().len(), 2);
    assert_eq!(
        Metronome::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&Metronome::default()).unwrap();
    assert_eq!(checked, 2);
}
