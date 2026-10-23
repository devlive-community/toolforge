use super::*;

#[test]
fn detects_json() {
    assert_eq!(detect("{\"a\": 1}").unwrap(), Detection::new(85, "object"));
    assert_eq!(detect("[1, 2]").unwrap().label, "array");
    assert_eq!(detect("{a: 1}").unwrap(), Detection::new(55, "invalid"));
    assert!(detect("[section]\nkey = 1").is_none());
    assert!(detect("hello").is_none());
}
