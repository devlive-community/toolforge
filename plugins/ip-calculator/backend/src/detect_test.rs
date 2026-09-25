use super::*;

#[test]
fn detects_addresses() {
    assert_eq!(detect("192.168.1.0/24").unwrap().label, "cidr");
    assert_eq!(
        detect("10.0.0.1").unwrap(),
        Detection::new(90, "ip").with("input", "10.0.0.1")
    );
    assert_eq!(detect("2001:db8::/32").unwrap().label, "cidr");
    assert!(detect("999.1.1.1").is_none());
    assert!(detect("1.2.3").is_none());
    assert!(detect("deadbeef").is_none());
}
