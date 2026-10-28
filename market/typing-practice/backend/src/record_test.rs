use super::*;

fn round(cpm: f64, accuracy: f64) -> Round {
    Round {
        cpm: Some(cpm),
        wpm: Some(cpm / 5.0),
        accuracy: Some(accuracy),
        active_ms: 30_000,
        passed: accuracy >= 0.9,
        pasted: false,
    }
}

#[test]
fn first_clean_round_becomes_the_best_and_clears_the_stage() {
    let out = settle(
        BTreeMap::new(),
        "b",
        true,
        &round(120.0, 0.95),
        "2026-10-28T00:00:00Z",
    );
    assert!(out.recorded && out.new_best && out.cleared);
    assert_eq!(out.bests["b"].cpm, Some(120.0));
}

#[test]
fn slower_round_keeps_the_previous_best() {
    let first = settle(BTreeMap::new(), "b", false, &round(120.0, 0.95), "t1");
    let out = settle(first.bests, "b", false, &round(100.0, 1.0), "t2");
    assert!(out.recorded);
    assert!(!out.new_best);
    assert_eq!(out.bests["b"].at, "t1");
}

#[test]
fn equal_speed_breaks_ties_by_accuracy() {
    let first = settle(BTreeMap::new(), "b", false, &round(120.0, 0.9), "t1");
    let out = settle(first.bests, "b", false, &round(120.0, 0.95), "t2");
    assert!(out.new_best);
}

#[test]
fn pasted_round_neither_records_nor_clears() {
    let mut r = round(500.0, 1.0);
    r.pasted = true;
    let out = settle(BTreeMap::new(), "b", true, &r, "t");
    assert!(!out.recorded && !out.new_best && !out.cleared);
    assert!(out.bests.is_empty());
}

#[test]
fn round_without_speed_is_not_recorded() {
    let mut r = round(0.0, 1.0);
    r.cpm = None;
    let out = settle(BTreeMap::new(), "b", false, &r, "t");
    assert!(!out.recorded);
    assert!(out.bests.is_empty());
}

#[test]
fn low_accuracy_does_not_clear_but_still_records() {
    let out = settle(BTreeMap::new(), "b", true, &round(80.0, 0.5), "t");
    assert!(out.recorded);
    assert!(!out.cleared);
}

#[test]
fn custom_text_never_clears_stages() {
    let out = settle(BTreeMap::new(), "custom", false, &round(80.0, 1.0), "t");
    assert!(!out.cleared);
}

#[test]
fn bests_are_capped_by_evicting_the_oldest() {
    let mut bests = BTreeMap::new();
    for i in 0..MAX_BESTS {
        let key = format!("k{i:03}");
        bests = settle(bests, &key, false, &round(100.0, 1.0), &format!("t{i:03}")).bests;
    }
    let out = settle(bests, "new", false, &round(100.0, 1.0), "t999");
    assert_eq!(out.bests.len(), MAX_BESTS);
    assert!(out.bests.contains_key("new"));
    assert!(!out.bests.contains_key("k000"), "最早的纪录被淘汰");
}
