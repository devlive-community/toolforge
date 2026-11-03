use super::*;
use serde_json::json;

#[test]
fn counts_cjk_and_latin_differently() {
    let out = count(Args {
        text: "你好世界".into(),
        model: Some("gpt-4o".into()),
        input_price: None,
        output_price: None,
        expected_output: None,
    })
    .unwrap();
    assert_eq!(out.cjk_chars, 4);
    // 4 个中文字 × 0.75
    assert_eq!(out.tokens, 3);
    assert_eq!(out.characters, 4);

    let out = count(Args {
        text: "hello world".into(),
        model: None,
        input_price: None,
        output_price: None,
        expected_output: None,
    })
    .unwrap();
    assert_eq!(out.latin_words, 2);
    assert!(
        out.tokens >= 2 && out.tokens <= 6,
        "latin estimate stays in a sane band"
    );
}

#[test]
fn digits_group_per_three() {
    let out = count(Args {
        text: "123456789".into(),
        model: None,
        input_price: None,
        output_price: None,
        expected_output: None,
    })
    .unwrap();
    assert_eq!(out.digit_groups, 3);
}

#[test]
fn unknown_model_falls_back_to_generic_coefficient() {
    let out = count(Args {
        text: "你好世界".into(),
        model: Some("my-model".into()),
        input_price: None,
        output_price: None,
        expected_output: None,
    })
    .unwrap();
    assert_eq!(out.model, "my-model");
    assert_eq!(out.method, "approxGeneric");
    assert_eq!(out.tokens, 3, "0.7 per CJK char rounds up to 3");
}

#[test]
fn cost_uses_user_prices_per_million() {
    let out = count(Args {
        text: "hello".into(),
        model: None,
        input_price: Some(3.0),
        output_price: Some(15.0),
        expected_output: Some(1_000_000),
    })
    .unwrap();
    let cost = out.cost.unwrap();
    let input = 3.0 * out.tokens as f64 / 1e6;
    assert!((cost.input - input).abs() < 1e-9);
    assert!((cost.output - 15.0).abs() < 1e-9);
    assert!((cost.total - (input + 15.0)).abs() < 1e-9);
}

#[test]
fn window_usage_ratio() {
    let out = count(Args {
        text: "a".repeat(128_000),
        model: Some("gpt-4o".into()),
        input_price: None,
        output_price: None,
        expected_output: None,
    })
    .unwrap();
    assert_eq!(out.context_window, 128_000);
    assert!(out.window_usage > 0.01 && out.window_usage < 0.5);
}

#[test]
fn empty_input_rejected() {
    let err = count(Args {
        text: String::new(),
        model: None,
        input_price: None,
        output_price: None,
        expected_output: None,
    })
    .unwrap_err();
    assert_eq!(err.code, "token.empty_input");
}

#[test]
fn dispatches_via_plugin() {
    let out = TokenCounter::default()
        .call("count", json!({"text": "你好"}))
        .unwrap();
    assert!(out["tokens"].as_u64().unwrap() >= 1);
    assert_eq!(
        TokenCounter::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&TokenCounter::default()).unwrap();
    assert_eq!(checked, 1);
}
