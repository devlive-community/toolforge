use super::*;

#[test]
fn same_text_gives_same_fingerprint() {
    assert_eq!(fingerprint("abc"), fingerprint("abc"));
}

#[test]
fn different_text_gives_different_fingerprint() {
    assert_ne!(fingerprint("abc"), fingerprint("abd"));
}

#[test]
fn fingerprint_is_short_fixed_hex() {
    let f = fingerprint("你好，世界");
    assert_eq!(f.len(), 8);
    assert!(f.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn empty_text_has_a_stable_fingerprint() {
    assert_eq!(fingerprint(""), fingerprint(""));
}
