use super::*;

#[test]
fn detects_identifiers() {
    let d = detect("550e8400-e29b-41d4-a716-446655440000").unwrap();
    assert_eq!(d, Detection::new(95, "uuid").with("version", 4));
    assert!(detect("{550e8400-e29b-41d4-a716-446655440000}").is_some());
    assert!(detect("550e8400e29b41d4a716446655440000").is_none());
    assert_eq!(detect("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap().label, "ulid");
    assert!(detect("hello").is_none());
}
