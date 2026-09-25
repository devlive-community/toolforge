use super::*;

#[test]
fn detects_domains() {
    assert_eq!(detect("github.com").unwrap(), Detection::new(60, "domain"));
    assert!(detect("mail.example.co.uk.").is_some());
    assert!(detect("readme.md").is_none());
    assert!(detect("1.2.3.4").is_none());
    assert!(detect("localhost").is_none());
    assert!(detect("-bad.com").is_none());
    assert!(detect("https://github.com").is_none());
}
