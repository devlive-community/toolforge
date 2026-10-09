use super::*;
use crate::number::parse_arabic;

fn n(input: &str) -> Number {
    parse_arabic(input).unwrap().unwrap()
}

#[test]
fn spells_numbers() {
    assert_eq!(words(&n("0")).unwrap(), "zero");
    assert_eq!(words(&n("15")).unwrap(), "fifteen");
    assert_eq!(words(&n("40")).unwrap(), "forty");
    assert_eq!(
        words(&n("1234")).unwrap(),
        "one thousand two hundred thirty-four"
    );
    assert_eq!(words(&n("1000001")).unwrap(), "one million one");
    assert_eq!(words(&n("-3.05")).unwrap(), "minus three point zero five");
    assert_eq!(words(&n("1e33")).unwrap(), "one decillion");
    assert_eq!(words(&n("1e36")).unwrap_err().code, "num.too_large_english");
}

#[test]
fn writes_check_amounts() {
    assert_eq!(
        check(&n("1234.5")).unwrap(),
        (
            "One Thousand Two Hundred Thirty-Four and 50/100".to_owned(),
            false
        )
    );
    assert_eq!(
        check(&n("0.999")).unwrap(),
        ("One and 00/100".to_owned(), true)
    );
}
