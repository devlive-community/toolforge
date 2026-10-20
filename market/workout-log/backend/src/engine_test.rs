use super::*;
fn run(store: &mut BTreeMap<String, Value>, action: Value) -> Value {
    let head = store.get("head").cloned().unwrap_or(Value::Null);
    let mut records = BTreeMap::new();
    for _ in 0..8 {
        let result = plan(
            serde_json::from_value(json!({"head":head,"records":records,"action":action})).unwrap(),
        )
        .unwrap();
        if result.need.is_empty() {
            for (k, v) in result.writes {
                store.insert(k, v);
            }
            return result.data;
        }
        for key in result.need {
            records.insert(key.clone(), store.get(&key).cloned().unwrap_or(Value::Null));
        }
    }
    panic!("read plan did not converge");
}
fn setup() -> BTreeMap<String, Value> {
    let mut s = BTreeMap::new();
    run(
        &mut s,
        json!({"op":"exercise","id":null,"name":"Lift","equipment":"Cable","mode":"external"}),
    );
    run(
        &mut s,
        json!({"op":"start","name":"Session","date":"2025-05-01","template":null}),
    );
    run(
        &mut s,
        json!({"op":"addExercise","workout":1,"exercise":"exercise-1"}),
    );
    s
}
fn set(status: &str, warmup: bool) -> Value {
    json!({"op":"set","workout":1,"entry":0,"index":0,"value":{"reps":"8","weight":"100","unit":"lb","warmup":warmup,"note":"actual","status":status}})
}
#[test]
fn completed_sets_are_idempotent_copies_pending_and_undo_restores() {
    let mut s = setup();
    run(&mut s, set("done", false));
    let before = s.clone();
    run(&mut s, set("done", false));
    assert_eq!(s, before);
    let d = run(
        &mut s,
        json!({"op":"addSet","workout":1,"entry":0,"copy":true}),
    );
    assert_eq!(d["summary"]["completed"], 1);
    assert_eq!(
        d["workout"]["body"]["entries"][0]["sets"][1]["status"],
        "pending"
    );
    let d = run(&mut s, json!({"op":"undo","workout":1}));
    assert_eq!(
        d["workout"]["body"]["entries"][0]["sets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn snapshots_survive_exercise_changes_and_template_edits() {
    let mut s = setup();
    run(&mut s, set("done", false));
    run(
        &mut s,
        json!({"op":"saveTemplate","workout":1,"id":null,"name":"My routine"}),
    );
    run(
        &mut s,
        json!({"op":"exercise","id":"exercise-1","name":"Renamed","equipment":"Other","mode":"bodyweight"}),
    );
    run(&mut s, json!({"op":"archive","id":"exercise-1"}));
    let d = run(
        &mut s,
        json!({"op":"start","name":"Next","date":"2025-05-02","template":"template-2"}),
    );
    assert_eq!(
        d["workout"]["body"]["entries"][0]["exercise"]["name"],
        "Lift"
    );
    assert_eq!(
        d["workout"]["body"]["entries"][0]["sets"][0]["status"],
        "pending"
    );
    let d = run(&mut s, json!({"op":"get","workout":1}));
    assert_eq!(
        d["workout"]["body"]["entries"][0]["exercise"]["equipment"],
        "Cable"
    );
}
#[test]
fn warmups_unknown_load_and_actual_dates_do_not_corrupt_totals() {
    let mut s = setup();
    run(&mut s, set("done", true));
    let d = run(&mut s, json!({"op":"get","workout":1}));
    assert_eq!(d["summary"]["completed"], 0);
    let d = run(&mut s, json!({"op":"get","workout":1,"warmups":true}));
    assert_eq!(d["summary"]["completed"], 1);
    assert_eq!(d["summary"]["entries"][0]["sets"][0]["kg"], "45.359");
    run(
        &mut s,
        json!({"op":"details","workout":1,"name":"Backdated","date":"2024-02-29","note":""}),
    );
    let d = run(
        &mut s,
        json!({"op":"list","date":"2025-05-01","exercise":null}),
    );
    assert_eq!(d["total"], 0);
    let d = run(
        &mut s,
        json!({"op":"list","date":"2024-02-29","exercise":null}),
    );
    assert_eq!(d["rows"][0]["workout"]["body"]["name"], "Backdated");
}
#[test]
fn history_is_sharded_without_a_total_retention_cap() {
    let mut s = setup();
    for n in 2..=140 {
        run(
            &mut s,
            json!({"op":"start","name":"Session","date":"2025-05-01","template":null}),
        );
        run(
            &mut s,
            json!({"op":"addExercise","workout":n,"exercise":"exercise-1"}),
        );
    }
    let d = run(
        &mut s,
        json!({"op":"list","offset":125,"limit":10,"exercise":"exercise-1","date":null}),
    );
    assert_eq!(d["total"], 140);
    assert_eq!(d["rows"].as_array().unwrap().len(), 10);
    assert!(s.contains_key("index-exercise-1-1"));
    assert!(s.contains_key("workout-1"));
}

#[test]
fn library_pages_are_independent_and_history_choices_include_archived_exercises() {
    let mut s = setup();
    for n in 2..=12 {
        run(
            &mut s,
            json!({"op":"exercise","id":null,"name":format!("Movement {n}"),"equipment":"","mode":"bodyweight"}),
        );
    }
    run(&mut s, json!({"op":"archive","id":"exercise-1"}));
    for n in 1..=12 {
        run(
            &mut s,
            json!({"op":"saveTemplate","workout":1,"id":null,"name":format!("Routine {n}")}),
        );
    }
    let d = run(
        &mut s,
        json!({"op":"home","offset":10,"templateOffset":0,"limit":10}),
    );
    assert_eq!(d["exercises"].as_array().unwrap().len(), 2);
    assert_eq!(d["templates"].as_array().unwrap().len(), 10);
    assert_eq!(d["templates"][0]["name"], "Routine 1");
    let choices = d["historyChoices"].as_array().unwrap();
    assert_eq!(choices.len(), 12);
    assert_eq!(choices[0]["id"], "exercise-1");
    assert_eq!(choices[0]["archived"], true);
    assert!(choices[0].get("history").is_none());
    assert!(
        d["choices"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"] != "exercise-1")
    );

    let d = run(
        &mut s,
        json!({"op":"home","offset":0,"templateOffset":10,"limit":10}),
    );
    assert_eq!(d["exercises"].as_array().unwrap().len(), 10);
    assert_eq!(d["templates"].as_array().unwrap().len(), 2);
    assert_eq!(d["templates"][0]["name"], "Routine 11");
    let d = run(
        &mut s,
        json!({"op":"list","exercise":"exercise-1","date":null}),
    );
    assert_eq!(d["rows"][0]["workout"]["id"], 1);
}

#[test]
fn a_template_offset_obeys_the_same_paging_budget() {
    let result = plan(serde_json::from_value(json!({
        "head":null,"records":{},"action":{"op":"home","offset":0,"templateOffset":MAX_SAFE+1,"limit":10}
    })).unwrap());
    assert_eq!(result.err().unwrap().params["field"], "page");
}
