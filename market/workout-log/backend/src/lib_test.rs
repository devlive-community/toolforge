use super::*;
#[test]
fn plugin_contract_and_clock_are_local() {
    let p = WorkoutLog::default();
    assert_eq!(p.manifest().id, "org.devlive.toolforge.workout-log");
    let c = p.call("clock", json!({"rest":null})).unwrap();
    assert!(c["remaining"].is_null());
    assert!(c["today"].as_str().unwrap().len() == 10);
    assert!(p.call("not_a_function", json!({})).is_err());
}

#[test]
fn clock_rejects_corrupt_rest_state_and_keeps_elapsed_sessions_complete() {
    let plugin = WorkoutLog::default();
    for (started, seconds) in [(i64::MAX, 90), (-1, 90), (0, 0), (0, 3601)] {
        let error = plugin
            .call(
                "clock",
                json!({"rest":{"started":started,"seconds":seconds,"stopped":false}}),
            )
            .unwrap_err();
        assert_eq!(error.code, "workout.invalid");
    }
    let clock = plugin
        .call(
            "clock",
            json!({"rest":{"started":1,"seconds":90,"stopped":false}}),
        )
        .unwrap();
    assert_eq!(clock["remaining"], 0);
    let clock = plugin
        .call(
            "clock",
            json!({"rest":{"started":1,"seconds":90,"stopped":true}}),
        )
        .unwrap();
    assert!(clock["remaining"].is_null());
}
