use super::*;

#[test]
fn compares_versions_numerically() {
    assert!(at_least("0.2.0", "0.1.10"));
    assert!(at_least("0.1.10", "0.1.9"));
    assert!(at_least("1.0", "1.0.0"));
    assert!(!at_least("0.1.9", "0.1.10"));
}
