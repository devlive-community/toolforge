use super::*;

#[test]
fn detects_statements() {
    assert_eq!(
        detect("select id, name from users where id = 1").unwrap(),
        Detection::new(85, "statement")
    );
    assert!(detect("INSERT INTO t VALUES (1)").is_some());
    assert!(detect("update the docs please").is_none());
    assert!(detect("with love").is_none());
    assert!(detect("hello from me").is_none());
}
