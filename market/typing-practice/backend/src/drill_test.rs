use super::*;

#[test]
fn builds_text_only_from_requested_characters() {
    let out = build("abc", 40, 7);
    assert_eq!(out.chars().count(), 40);
    assert!(
        out.chars().all(|c| "abc ".contains(c)),
        "unexpected char in {out:?}"
    );
}

#[test]
fn same_seed_gives_same_text() {
    assert_eq!(build("xy", 30, 42), build("xy", 30, 42));
}

#[test]
fn empty_or_space_only_input_yields_nothing() {
    assert_eq!(build("", 20, 1), "");
    assert_eq!(build("  ", 20, 1), "");
}

#[test]
fn length_is_clamped_to_limit() {
    assert_eq!(build("a", 10_000, 1).chars().count(), MAX_LEN);
}

#[test]
fn duplicate_characters_are_collapsed() {
    let out = build("aaaa", 12, 3);
    assert!(out.chars().all(|c| c == 'a' || c == ' '));
}

#[test]
fn multibyte_chars_are_handled_as_units() {
    let out = build("你好", 12, 9);
    assert_eq!(out.chars().count(), 12);
    assert!(out.chars().all(|c| "你好 ".contains(c)));
}
