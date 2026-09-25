use super::*;

#[test]
fn detects_commands() {
    assert_eq!(
        detect("curl https://a.b").unwrap(),
        Detection::new(99, "command")
    );
    assert!(detect("$ /usr/bin/curl -I x").is_some());
    assert!(detect("curly braces").is_none());
    assert!(detect("wget x").is_none());
}
