use super::*;
use serde_json::json;

#[test]
fn score_function_accepts_flat_args_and_returns_session_and_score() {
    let plugin = TypingPractice::default();
    let out = plugin
        .call(
            "score",
            json!({
                "target": "abc",
                "typed": "abc",
                "elapsedMs": 60000
            }),
        )
        .expect("score should succeed");
    assert_eq!(out["score"]["completed"], json!(true));
    assert_eq!(out["session"]["attempts"], json!(3));
}

#[test]
fn session_can_be_passed_as_json_string_for_cli_and_agent() {
    let plugin = TypingPractice::default();
    let first = plugin
        .call("score", json!({ "target": "abc", "typed": "x" }))
        .expect("first call");
    let carried = first["session"].to_string();
    let second = plugin
        .call(
            "score",
            json!({ "target": "abc", "prev": "", "typed": "a", "session": carried }),
        )
        .expect("second call with string session");
    assert_eq!(second["session"]["attempts"], json!(2));
    assert_eq!(
        second["session"]["wrongAttempts"],
        json!(1),
        "纠正不抹掉已发生的错误"
    );
}

#[test]
fn malformed_session_string_is_rejected_with_invalid_args() {
    let plugin = TypingPractice::default();
    let err = plugin
        .call(
            "score",
            json!({ "target": "abc", "typed": "", "session": "{not json" }),
        )
        .expect_err("malformed session must be rejected");
    assert_eq!(err.code, "plugin.invalid_args");
}

#[test]
fn unknown_function_is_rejected() {
    let plugin = TypingPractice::default();
    assert!(plugin.call("nope", json!({})).is_err());
}

#[test]
fn declared_interfaces_are_accepted_by_backend() {
    // 宿主视角：声明的 CLI 接口用示例参数调用后，后端必须接受（不能 invalid_args）
    let plugin = TypingPractice::default();
    let checked = tf_plugin_api::interface::check_declared_interfaces(&plugin)
        .expect("declared interfaces must match the backend");
    assert!(checked >= 2, "score and drill should be checked");
}

#[test]
fn prepare_splits_visible_characters_and_fingerprints_the_text() {
    let plugin = TypingPractice::default();
    let out = plugin
        .call("prepare", json!({ "text": "a👍🏽b" }))
        .expect("prepare");
    assert_eq!(out["total"], json!(3));
    assert_eq!(out["graphemes"][1], json!("👍🏽"));
    let same = plugin.call("prepare", json!({ "text": "a👍🏽b" })).unwrap();
    let other = plugin.call("prepare", json!({ "text": "a👍🏽c" })).unwrap();
    assert_eq!(out["fingerprint"], same["fingerprint"]);
    assert_ne!(out["fingerprint"], other["fingerprint"]);
}

#[test]
fn oversized_text_is_rejected() {
    let plugin = TypingPractice::default();
    let long = "a".repeat(score::MAX_TEXT + 1);
    let err = plugin
        .call("prepare", json!({ "text": long }))
        .expect_err("too long");
    assert_eq!(err.code, "typing.text_too_long");
    let err = plugin
        .call("score", json!({ "target": "ab", "typed": "a".repeat(500) }))
        .expect_err("typed far beyond target");
    assert_eq!(err.code, "typing.text_too_long");
}

#[test]
fn settle_records_a_clean_round_and_rejects_a_pasted_one() {
    let plugin = TypingPractice::default();
    let round =
        json!({ "cpm": 120.0, "wpm": 24.0, "accuracy": 1.0, "activeMs": 5000, "passed": true });
    let out = plugin
        .call(
            "settle",
            json!({ "bucket": "builtin:en:free:en-1:abcd", "stage": true, "round": round, "at": "t" }),
        )
        .expect("settle");
    assert_eq!(out["recorded"], json!(true));
    assert_eq!(out["cleared"], json!(true));
    let pasted = json!({ "cpm": 900.0, "accuracy": 1.0, "passed": true, "pasted": true });
    let again = plugin
        .call(
            "settle",
            json!({ "bests": out["bests"], "bucket": "builtin:en:free:en-1:abcd", "stage": true, "round": pasted, "at": "t2" }),
        )
        .expect("settle pasted");
    assert_eq!(again["recorded"], json!(false));
    assert_eq!(again["cleared"], json!(false));
    assert_eq!(
        again["bests"]["builtin:en:free:en-1:abcd"]["cpm"],
        json!(120.0)
    );
}
