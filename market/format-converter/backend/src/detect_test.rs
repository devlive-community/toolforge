use super::*;

#[test]
fn detects_documents() {
    assert_eq!(
        detect("name: app\nversion: 1").unwrap(),
        Detection::new(60, "yaml")
    );
    assert_eq!(detect("- a\n- b").unwrap().label, "yaml");
    assert_eq!(
        detect("[server]\nport = 80").unwrap(),
        Detection::new(70, "toml")
    );
    assert_eq!(detect("{\"a\": 1}").unwrap().label, "json");
    assert!(detect("just some words\nand more words").is_none());
    assert!(detect("key: value").is_none());
}
