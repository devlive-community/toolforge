use super::*;

fn n(input: &str) -> Number {
    parse_arabic(input).unwrap().unwrap()
}

#[test]
fn parses_arabic_forms() {
    assert_eq!(n("1,234.50"), Number::new(false, "1234", "5").unwrap());
    assert_eq!(n("-0.0"), Number::new(false, "0", "").unwrap());
    assert_eq!(n("1.5万").plain(), "15000");
    assert_eq!(n("3亿").plain(), "300000000");
    assert_eq!(n("2.5k").plain(), "2500");
    assert_eq!(n("1.2e-3").plain(), "0.0012");
    assert_eq!(n("6.02E23").plain(), "602000000000000000000000");
    assert_eq!(n(".5").plain(), "0.5");
    assert!(parse_arabic("一千").is_none());
    assert!(parse_arabic("12a").is_none());
    assert!(parse_arabic("1e5000").unwrap().is_err());
}

#[test]
fn rounds_half_up_with_carry() {
    assert_eq!(n("1.005").round(2).plain(), "1.01");
    assert_eq!(n("9.995").round(2).plain(), "10");
    assert_eq!(n("0.994").round(2).plain(), "0.99");
    assert_eq!(n("12").round(2).plain(), "12");
}

#[test]
fn formats_groups_and_scientific_notation() {
    let x = n("-1234567.89");
    assert_eq!(x.grouped(3, ','), "-1,234,567.89");
    assert_eq!(x.grouped(3, ' '), "-1 234 567.89");
    assert_eq!(n("123456789").indian(), "12,34,56,789");
    assert_eq!(n("999").indian(), "999");
    assert_eq!(x.scientific(), "-1.23456789e+6");
    assert_eq!(n("0.00012").scientific(), "1.2e-4");
    assert_eq!(n("5").scientific(), "5e+0");
    assert_eq!(n("0").scientific(), "0");
}
