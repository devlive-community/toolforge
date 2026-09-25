use super::*;

#[test]
fn detects_prefixed_numbers() {
    assert_eq!(
        detect("0xDEADBEEF").unwrap(),
        Detection::new(80, "number").with("base", 16)
    );
    assert_eq!(detect("0b1010").unwrap().params["base"], 2);
    assert_eq!(detect("-0o17").unwrap().params["base"], 8);
    assert!(detect("0x").is_none());
    assert!(detect("0xZZ").is_none());
    assert!(detect("1234").is_none());
}
