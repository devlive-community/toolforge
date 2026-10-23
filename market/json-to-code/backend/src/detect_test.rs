use super::*;

#[test]
fn detects_objects_only() {
    assert_eq!(detect("{\"id\": 1}").unwrap(), Detection::new(50, "model"));
    assert!(detect("[{\"id\": 1}]").is_some());
    assert!(detect("[1, 2]").is_none());
    assert!(detect("{}").is_none());
    assert!(detect("{oops").is_none());
}
