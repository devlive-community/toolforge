use super::*;

#[test]
fn detects_documents() {
    assert_eq!(
        detect("<a><b>1</b></a>").unwrap(),
        Detection::new(85, "document")
    );
    assert!(detect("<?xml version=\"1.0\"?><root/>").is_some());
    assert!(detect("<a><b></a>").is_none());
    assert!(detect("a < b > c").is_none());
}
