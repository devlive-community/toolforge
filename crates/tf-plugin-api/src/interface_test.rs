use serde_json::json;

use super::*;
use crate::Manifest;

fn declared() -> Value {
    json!({
        "expose": ["cli", "agent"],
        "summary": "i18n:api.convert.summary",
        "effect": "read",
        "params": { "type": "object", "properties": {} }
    })
}

#[test]
fn missing_and_false_are_distinct() {
    assert_eq!(Declaration::parse(None), Declaration::Undeclared);
    assert_eq!(
        Declaration::parse(Some(&Value::Null)),
        Declaration::Undeclared
    );
    assert_eq!(Declaration::parse(Some(&json!(false))), Declaration::Hidden);
    assert_eq!(Declaration::parse(Some(&json!(true))), Declaration::Invalid);
}

#[test]
fn parses_declaration_and_exposes_channels() {
    let declaration = Declaration::parse(Some(&declared()));
    let interface = declaration.exposed(Channel::Cli).unwrap();
    assert_eq!(interface.effect, Effect::Read);
    assert!(interface.uses.is_empty());
    assert!(declaration.exposed(Channel::Agent).is_some());
}

#[test]
fn unknown_channels_are_ignored() {
    let mut raw = declared();
    raw["expose"] = json!(["cli", "future"]);
    let declaration = Declaration::parse(Some(&raw));
    assert!(declaration.exposed(Channel::Cli).is_some());
    assert!(declaration.exposed(Channel::Agent).is_none());
}

#[test]
fn unknown_effect_hides_the_function() {
    let mut raw = declared();
    raw["effect"] = json!("teleport");
    assert_eq!(Declaration::parse(Some(&raw)), Declaration::Invalid);
}

#[test]
fn malformed_interface_does_not_break_manifest() {
    let manifest = Manifest::from_static(
        r#"{"id":"a","version":"1.0.0","name":"n","description":"d","category":"dev",
            "functions":{"run":{"interface":{"expose":"cli"}},"other":{"interface":false}}}"#,
    );
    assert_eq!(
        manifest.functions["run"].declaration(),
        Declaration::Invalid
    );
    assert_eq!(
        manifest.functions["other"].declaration(),
        Declaration::Hidden
    );
}
