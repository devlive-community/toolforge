use super::*;
use serde_json::json;

#[test]
fn rounds_stay_within_bounds_and_reproduce() {
    for seed in [1u64, 42, 999] {
        let out = reaction_round(RoundArgs {
            seed,
            min_ms: None,
            max_ms: None,
        })
        .unwrap();
        assert!((1000..=4000).contains(&out.wait_ms));
        let again = reaction_round(RoundArgs {
            seed,
            min_ms: None,
            max_ms: None,
        })
        .unwrap();
        assert_eq!(out.wait_ms, again.wait_ms, "seed {seed} reproduces");
    }
    let err = reaction_round(RoundArgs {
        seed: 1,
        min_ms: Some(5000),
        max_ms: Some(1000),
    })
    .unwrap_err();
    assert_eq!(err.code, "reaction.bad_wait");
}

#[test]
fn scores_median_and_official_threshold() {
    let out = reaction_score(ScoreArgs {
        times_ms: vec![Some(300), Some(250), None, Some(400), Some(280)],
    })
    .unwrap();
    assert_eq!(out.valid_count, 4);
    assert_eq!(out.false_starts, 1);
    assert_eq!(
        out.median_ms,
        Some(300),
        "sorted [250,280,300,400] takes index 2"
    );
    assert!(out.official);
    assert_eq!(out.best_ms, Some(250));
    // 不足 3 个有效轮不记正式成绩
    let out = reaction_score(ScoreArgs {
        times_ms: vec![Some(300), None, Some(350), None, None],
    })
    .unwrap();
    assert!(!out.official);
    // 空成绩合法但无中位数
    let out = reaction_score(ScoreArgs {
        times_ms: vec![None, None, None, None, None],
    })
    .unwrap();
    assert_eq!(out.median_ms, None);
    assert!(!out.official);
}

#[test]
fn sequence_lengths_and_judgement() {
    let out = sequence(SequenceArgs {
        seed: 9,
        length: 6,
        pads: Some(4),
    })
    .unwrap();
    assert_eq!(out.order.len(), 6);
    assert!(out.order.iter().all(|&p| p < 4));
    let again = sequence(SequenceArgs {
        seed: 9,
        length: 6,
        pads: Some(4),
    })
    .unwrap();
    assert_eq!(out.order, again.order);
    let err = sequence(SequenceArgs {
        seed: 1,
        length: 40,
        pads: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "reaction.bad_length");

    // 完全一致 → passed
    let out = sequence_judge(JudgeArgs {
        expected: vec![0, 1, 2],
        entered: vec![0, 1, 2],
    })
    .unwrap();
    assert_eq!(out.status, "passed");
    // 第 3 项错误 → failed + 位置
    let out = sequence_judge(JudgeArgs {
        expected: vec![0, 1, 2],
        entered: vec![0, 1, 3],
    })
    .unwrap();
    assert_eq!(out.status, "failed");
    assert_eq!(out.correct_len, 2);
    // 输入不足 → failed
    let out = sequence_judge(JudgeArgs {
        expected: vec![0, 1, 2],
        entered: vec![0],
    })
    .unwrap();
    assert_eq!(out.status, "failed");
    assert_eq!(out.correct_len, 1);
}

#[test]
fn dispatches_via_plugin() {
    let tool = ReactionChallenge::default();
    let out = tool
        .call("sequence", json!({"seed": 1, "length": 3}))
        .unwrap();
    assert_eq!(out["order"].as_array().unwrap().len(), 3);
    let out = tool
        .call(
            "reaction_score",
            json!({"timesMs": [250, 300, 280, null, null]}),
        )
        .unwrap();
    assert_eq!(out["official"], true);
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&ReactionChallenge::default()).unwrap();
    assert_eq!(checked, 4);
}
