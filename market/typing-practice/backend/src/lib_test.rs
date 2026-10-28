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
                "added": 3,
                "wrongAdded": 0,
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
        .call(
            "score",
            json!({ "target": "abc", "typed": "x", "added": 1, "wrongAdded": 1 }),
        )
        .expect("first call");
    let carried = first["session"].to_string();
    let second = plugin
        .call(
            "score",
            json!({ "target": "abc", "typed": "a", "added": 1, "wrongAdded": 0, "session": carried }),
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
    assert!(checked >= 1, "score should be checked");
}
