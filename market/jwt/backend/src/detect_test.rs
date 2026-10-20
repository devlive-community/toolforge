use super::*;

const TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.sig";

#[test]
fn detects_tokens() {
    assert_eq!(
        detect(TOKEN).unwrap(),
        Detection::new(98, "token").with("alg", "HS256")
    );
    assert!(detect("a.b.c").is_none());
    assert!(detect("eyJ9.eyJ9").is_none());
    assert!(detect("www.example.com").is_none());
}
