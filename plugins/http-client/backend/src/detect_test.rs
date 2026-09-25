use super::*;

#[test]
fn detects_urls() {
    assert_eq!(
        detect("https://api.github.com/repos").unwrap(),
        Detection::new(70, "url").with("host", "api.github.com")
    );
    assert!(detect("http://localhost:8080?a=1").is_some());
    assert!(detect("https://").is_none());
    assert!(detect("https://a.b c").is_none());
    assert!(detect("ftp://a.b").is_none());
}
