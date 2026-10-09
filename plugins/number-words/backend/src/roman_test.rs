use super::*;

#[test]
fn converts_both_ways() {
    assert_eq!(to_roman(1994).as_deref(), Some("MCMXCIV"));
    assert_eq!(to_roman(3999).as_deref(), Some("MMMCMXCIX"));
    assert_eq!(to_roman(0), None);
    assert_eq!(to_roman(4000), None);
    assert_eq!(parse("mcmxciv"), Some(1994));
    assert_eq!(parse("IIII"), None, "not canonical");
    assert_eq!(parse("IC"), None);
    assert_eq!(parse("hello"), None);
    for n in 1..=3999 {
        assert_eq!(parse(&to_roman(n).unwrap()), Some(n));
    }
}
