use super::*;
use serde_json::json;

#[test]
fn sticks_are_reproducible_and_valid() {
    let a = stick(StickArgs {
        seed: 2026,
        category: None,
    })
    .unwrap();
    let b = stick(StickArgs {
        seed: 2026,
        category: None,
    })
    .unwrap();
    assert_eq!(a.title, b.title, "same seed draws the same stick");
    assert!(matches!(a.level, "upper" | "mid" | "lower"));
    assert!(!a.text_zh.is_empty());
    assert!(!a.text_en.is_empty());
    assert!(a.index >= 1 && a.index <= STICKS.len() as u64);
    let err = stick(StickArgs {
        seed: 1,
        category: Some("other".into()),
    })
    .unwrap_err();
    assert_eq!(err.code, "fortune.bad_category");
}

#[test]
fn tarot_spreads_use_distinct_cards() {
    let out = tarot(TarotArgs {
        seed: 7,
        spread: Some("three".into()),
    })
    .unwrap();
    assert_eq!(out.cards.len(), 3);
    let keys: Vec<&str> = out.cards.iter().map(|c| c.key).collect();
    assert_ne!(keys[0], keys[1]);
    assert_ne!(keys[1], keys[2]);
    assert_eq!(out.cards[0].position, "past");
    assert_eq!(out.cards[2].position, "future");
    let single = tarot(TarotArgs {
        seed: 7,
        spread: Some("single".into()),
    })
    .unwrap();
    assert_eq!(single.cards.len(), 1);
    let err = tarot(TarotArgs {
        seed: 7,
        spread: Some("cross".into()),
    })
    .unwrap_err();
    assert_eq!(err.code, "fortune.bad_spread");
}

#[test]
fn daily_card_is_stable_per_date() {
    let a = daily(DailyArgs { date: 20261103 }).unwrap();
    let b = daily(DailyArgs { date: 20261103 }).unwrap();
    assert_eq!(a.card.key, b.card.key);
    assert_eq!(a.level, b.level);
    let other = daily(DailyArgs { date: 20261104 }).unwrap();
    assert_ne!(a.seed, other.seed);
    let err = daily(DailyArgs { date: 42 }).unwrap_err();
    assert_eq!(err.code, "fortune.bad_date");
}

#[test]
fn dispatches_via_plugin() {
    let tool = FortuneTarot::default();
    let out = tool.call("stick", json!({"seed": 42})).unwrap();
    assert!(out["level"].is_string());
    let out = tool.call("daily", json!({"date": 20261103})).unwrap();
    assert!(out["card"]["nameZh"].is_string());
    assert_eq!(
        tool.call("nope", json!({})).unwrap_err().code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&FortuneTarot::default()).unwrap();
    assert_eq!(checked, 3);
}
