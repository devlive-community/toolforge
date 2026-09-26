use super::*;

#[test]
fn detects_endpoints() {
    let d = detect("localhost:3000").unwrap();
    assert_eq!(
        d,
        Detection::new(60, "endpoint")
            .with("host", "localhost")
            .with("port", 3000)
    );
    assert_eq!(detect("[::1]:8080").unwrap().params["host"], "::1");
    assert_eq!(detect("10.0.0.5:22").unwrap().params["port"], 22);
    assert!(detect("https://a.b:443").is_none());
    assert!(detect("a:b").is_none());
    assert!(detect("key: value").is_none());
    assert!(detect("host:0").is_none());
    assert!(detect("12:30").is_none(), "times are not endpoints");
}
